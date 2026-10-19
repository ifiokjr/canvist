//! The canvas: state, tools, commands, and inspectors behind one input
//! entry point.

use std::sync::Arc;

use canvist_model::Schema;
use canvist_scene::NodeId;
use canvist_scene::Prop;
use canvist_scene::PropKey;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::SceneSchema;
use canvist_scene::SceneSchemaBuilder;
use canvist_scene::SceneSchemaError;
use kurbo::Point;
use rustc_hash::FxHashMap;
use serde_json::Value as Json;
use serde_json::json;

use crate::CanvasError;
use crate::CanvasState;
use crate::Cursor;
use crate::Input;
use crate::OverlayItem;
use crate::Pinch;
use crate::TextLayouts;
use crate::extension::CanvasExtension;
use crate::extension::Command;
use crate::extension::Registration;
use crate::extension::ToolId;
use crate::extension::TypedCanvasExtension;
use crate::input::KeyEvent;
use crate::input::Phase;
use crate::input::PointerEvent;
use crate::input::WheelEvent;
use crate::inspector;
use crate::inspector::Control;
use crate::inspector::Field;
use crate::inspector::Section;
use crate::overlay::OutlineKind;
use crate::tools::HandTool;
use crate::tools::Outcome;
use crate::tools::SelectTool;
use crate::tools::Tool;
use crate::tools::select::screen_outline;

/// How strongly a pinch or Ctrl-wheel zooms per pixel of scroll.
const ZOOM_PER_PIXEL: f64 = 0.01;

/// Collects extensions, then builds a [`Canvas`].
pub struct CanvasBuilder {
	text: Option<Arc<Schema>>,
	extensions: Vec<Arc<dyn CanvasExtension>>,
	tools: Vec<Box<dyn Tool>>,
}

impl std::fmt::Debug for CanvasBuilder {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CanvasBuilder")
			.field(
				"extensions",
				&self
					.extensions
					.iter()
					.map(|extension| extension.name())
					.collect::<Vec<_>>(),
			)
			.finish_non_exhaustive()
	}
}

impl Default for CanvasBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl CanvasBuilder {
	/// A builder with the select and hand tools.
	#[must_use]
	pub fn new() -> Self {
		Self {
			text: None,
			extensions: Vec::new(),
			tools: vec![Box::new(SelectTool::new()), Box::new(HandTool::new())],
		}
	}

	/// The schema of text frames' documents.
	#[must_use]
	pub fn text_schema(mut self, schema: Arc<Schema>) -> Self {
		self.text = Some(schema);
		self
	}

	/// Register `extension` and return its typed handle.
	pub fn add<E: TypedCanvasExtension>(&mut self, extension: E) -> E::Handle {
		let start = self.tools.len();
		self.tools.extend(extension.tools());
		let handle = extension.handle(Registration::new(start..self.tools.len()));
		self.extensions.push(Arc::new(extension));
		handle
	}

	/// The scene schema the registered extensions declare. Scenes the canvas
	/// edits, and collaboration bindings, use it.
	pub fn schema(&self) -> Result<Arc<SceneSchema>, SceneSchemaError> {
		let mut builder: SceneSchemaBuilder = SceneSchema::builder();

		if let Some(text) = &self.text {
			builder = builder.text(text.clone());
		}

		for extension in &self.extensions {
			builder = extension.schema(builder);
		}

		builder.build().map(Arc::new)
	}

	/// Build a canvas over `scene`, which must use [`CanvasBuilder::schema`]'s
	/// node types, laying out text with `texts`.
	pub fn build(self, scene: Scene, texts: TextLayouts) -> Result<Canvas, CanvasError> {
		let mut commands: FxHashMap<String, Command> = core_commands()
			.into_iter()
			.map(|command| (command.name.clone(), command))
			.collect();
		let mut sections = inspector::core_sections();

		for extension in &self.extensions {
			commands.extend(
				extension
					.commands()
					.into_iter()
					.map(|command| (command.name.clone(), command)),
			);
			sections.extend(extension.inspector());
		}

		Ok(Canvas {
			state: CanvasState::new(scene, texts)?,
			tools: self.tools,
			active: ToolId::SELECT,
			commands,
			sections,
			extensions: self.extensions,
			pointers: FxHashMap::default(),
			pinch: None,
		})
	}
}

/// A headless canvas editor.
///
/// Hosts feed it [`Input`] and paint the scene plus [`Canvas::overlay`]. It
/// owns the scene, its index, the camera, the selection, the tools, and
/// local history; collaboration bindings drain its commits with
/// [`CanvasState::take_commits`] and feed remote ones to
/// [`CanvasState::apply_remote`].
pub struct Canvas {
	state: CanvasState,
	tools: Vec<Box<dyn Tool>>,
	active: ToolId,
	commands: FxHashMap<String, Command>,
	sections: Vec<Section>,
	extensions: Vec<Arc<dyn CanvasExtension>>,
	pointers: FxHashMap<u64, Point>,
	pinch: Option<Pinch>,
}

impl std::fmt::Debug for Canvas {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Canvas")
			.field("state", &self.state)
			.field("active", &self.active)
			.finish_non_exhaustive()
	}
}

impl Canvas {
	/// Start building a canvas.
	#[must_use]
	pub fn builder() -> CanvasBuilder {
		CanvasBuilder::new()
	}

	/// The state tools work on.
	#[must_use]
	pub fn state(&self) -> &CanvasState {
		&self.state
	}

	/// The state, to change the camera, selection, or scene directly.
	pub fn state_mut(&mut self) -> &mut CanvasState {
		&mut self.state
	}

	/// The active tool.
	#[must_use]
	pub fn tool(&self) -> ToolId {
		self.active
	}

	/// The registered tools' names, indexed by [`ToolId::index`].
	#[must_use]
	pub fn tools(&self) -> Vec<&str> {
		self.tools.iter().map(|tool| tool.name()).collect()
	}

	/// The tool registered as `name`, such as `"rectangle"`.
	#[must_use]
	pub fn tool_named(&self, name: &str) -> Option<ToolId> {
		self.tools
			.iter()
			.position(|tool| tool.name() == name)
			.map(ToolId)
	}

	/// Switch tools, abandoning the current tool's gesture.
	pub fn set_tool(&mut self, tool: ToolId) {
		if tool == self.active || tool.0 >= self.tools.len() {
			return;
		}

		self.tools[self.active.0].cancel(&mut self.state);
		self.active = tool;
	}

	/// Handle one input event.
	///
	/// The wheel and two-finger touch drive the camera, the primary modifier
	/// with Z undoes (redoes with Shift), and everything else goes to the
	/// active tool.
	pub fn input(&mut self, input: &Input) -> Result<Outcome, CanvasError> {
		match input {
			Input::Wheel(wheel) => Ok(self.wheel(wheel)),

			Input::Pointer(pointer) if self.multi_touch(pointer) => Ok(Outcome::Redraw),

			Input::Key(key) if is_undo(key) => self.undo_or_redo(key.modifiers.shift),
			_ => self.tool_input(input),
		}
	}

	/// Advance time-based motion (camera momentum) by `elapsed` seconds.
	/// Returns whether the host should paint another frame.
	pub fn frame(&mut self, elapsed: f64) -> bool {
		match self.state.inertia.step(elapsed) {
			Some(delta) => {
				self.state.camera_mut().pan(delta);
				true
			}
			None => false,
		}
	}

	/// The overlay to draw over the scene, in screen pixels.
	#[must_use]
	pub fn overlay(&self) -> Vec<OverlayItem> {
		let mut out: Vec<OverlayItem> = self
			.state
			.selection()
			.nodes()
			.iter()
			.filter_map(|&id| screen_outline(&self.state, id))
			.map(|path| {
				OverlayItem::Outline {
					path,
					kind: OutlineKind::Selection,
				}
			})
			.collect();
		self.tools[self.active.0].overlay(&self.state, &mut out);
		out.extend(self.state.chrome.iter().cloned());
		out
	}

	/// The cursor the active tool wants.
	#[must_use]
	pub fn cursor(&self) -> Cursor {
		self.tools[self.active.0].cursor(&self.state)
	}

	/// Run command `name` with JSON `args`.
	pub fn run(&mut self, name: &str, args: &Json) -> Result<Outcome, CanvasError> {
		let command = self
			.commands
			.get(name)
			.map(|command| command.run.clone())
			.ok_or_else(|| CanvasError::UnknownCommand(name.to_owned()))?;
		command(&mut self.state, args)
	}

	/// The inspector for the selection (see [`inspector::inspect`]).
	#[must_use]
	pub fn inspect(&self) -> Json {
		inspector::inspect(&self.state, &self.sections)
	}

	/// Set an inspector field on every selected node.
	pub fn set_field(&mut self, key: &str, value: &Json) -> Result<(), CanvasError> {
		inspector::set_field(&mut self.state, key, value)
	}

	/// Machine-readable metadata: extensions, node types, tools, commands,
	/// and inspector sections. Bindings generate their types from it.
	#[must_use]
	pub fn metadata(&self) -> Json {
		let schema = self.state.scene().schema();
		let mut commands: Vec<&Command> = self.commands.values().collect();
		commands.sort_by(|a, b| a.name.cmp(&b.name));

		let extensions: Vec<&str> = self
			.extensions
			.iter()
			.map(|extension| extension.name())
			.collect();
		let nodes: Vec<Json> = schema
			.types()
			.map(|(_, spec)| {
				json!({
					"name": spec.name(),
					"kind": format!("{:?}", spec.kind()),
					"children": spec.has_children(),
				})
			})
			.collect();
		let sections: Vec<Json> = self
			.sections
			.iter()
			.map(|section| {
				json!({
					"id": section.id,
					"title": section.title,
					"fields": section.fields.iter().map(Field::meta).collect::<Vec<_>>(),
				})
			})
			.collect();

		json!({
			"extensions": extensions,
			"nodes": nodes,
			"tools": self.tools(),
			"commands": commands.into_iter().map(Command::meta).collect::<Vec<_>>(),
			"inspector": sections,
		})
	}

	/// Undo, or redo when `redo` is set; redraw only if history moved.
	fn undo_or_redo(&mut self, redo: bool) -> Result<Outcome, CanvasError> {
		let changed = if redo {
			self.state.redo()?
		} else {
			self.state.undo()?
		};

		Ok(if changed {
			Outcome::Redraw
		} else {
			Outcome::Idle
		})
	}

	/// Pass `input` to the active tool, returning to the select tool when it
	/// finishes a creation.
	fn tool_input(&mut self, input: &Input) -> Result<Outcome, CanvasError> {
		let outcome = self.tools[self.active.0].input(&mut self.state, input)?;

		if outcome == Outcome::Done {
			self.set_tool(ToolId::SELECT);
		}

		Ok(outcome)
	}

	fn wheel(&mut self, wheel: &WheelEvent) -> Outcome {
		let camera = self.state.camera_mut();

		if wheel.pinch || wheel.modifiers.primary {
			camera.zoom_at(wheel.position, (-wheel.delta.y * ZOOM_PER_PIXEL).exp());
		} else {
			camera.pan(-wheel.delta);
		}

		Outcome::Redraw
	}

	/// Track touches; two at once pinch-zoom the camera and cancel the
	/// tool's gesture. Returns whether the event was consumed.
	fn multi_touch(&mut self, event: &PointerEvent) -> bool {
		if !matches!(event.kind, crate::input::PointerKind::Touch) {
			return false;
		}

		match event.phase {
			Phase::Down | Phase::Move => {
				self.pointers.insert(event.id, event.position);
			}
			Phase::Up | Phase::Cancel => {
				self.pointers.remove(&event.id);
			}
		}

		let mut touches = self.pointers.values().copied();

		match (touches.next(), touches.next(), self.pinch.as_mut()) {
			(Some(a), Some(b), Some(pinch)) => {
				pinch.update(self.state.camera_mut(), a, b);
				true
			}
			(Some(a), Some(b), None) => {
				self.tools[self.active.0].cancel(&mut self.state);
				self.pinch = Some(Pinch::start(a, b));
				true
			}
			_ => self.pinch.take().is_some(),
		}
	}
}

/// Commands every canvas has: `insert`, `remove`, and `select`.
fn core_commands() -> Vec<Command> {
	vec![
		Command::new(
			"insert",
			"Insert a node of a registered type, selecting it.",
			vec![
				Field::new("type", "Type", Control::Text),
				Field::new("parent", "Parent", Control::Text),
				Field::new("props", "Props", Control::Json),
			],
			insert,
		),
		Command::new(
			"remove",
			"Remove nodes (the selection by default).",
			vec![Field::new("nodes", "Nodes", Control::Json)],
			|state, args| {
				let nodes =
					node_list(args, "nodes")?.unwrap_or_else(|| state.selection().nodes().to_vec());
				state.edit(false, |transaction| transaction.remove(&nodes))?;
				Ok(Outcome::Redraw)
			},
		),
		Command::new(
			"select",
			"Select nodes.",
			vec![Field::new("nodes", "Nodes", Control::Json)],
			|state, args| {
				let nodes = node_list(args, "nodes")?.unwrap_or_default();
				state.selection_mut().set(nodes);
				Ok(Outcome::Redraw)
			},
		),
	]
}

/// Whether `key` is the undo shortcut: the primary modifier with Z (Shift
/// turns it into redo).
fn is_undo(key: &KeyEvent) -> bool {
	key.down && key.modifiers.primary && key.key.eq_ignore_ascii_case("z")
}

/// The `insert` command: parse the node type, the parent (the first page
/// by default), and any props from `args`, insert the node, and select it.
fn insert(state: &mut CanvasState, args: &Json) -> Result<Outcome, CanvasError> {
	let kind = args
		.get("type")
		.and_then(Json::as_str)
		.ok_or_else(|| CanvasError::InvalidArgs("`type` must be a node type name".into()))?
		.to_owned();

	let parent = match args.get("parent").and_then(Json::as_str) {
		Some(parent) => {
			parent
				.parse()
				.map_err(|error| CanvasError::InvalidArgs(format!("{error}")))?
		}
		None => {
			state
				.scene()
				.pages()
				.first()
				.copied()
				.unwrap_or(NodeId::ROOT)
		}
	};

	let behaviour = state
		.scene()
		.schema()
		.node_type(&kind)
		.map(|id| state.scene().schema().spec(id).kind())
		.ok_or_else(|| CanvasError::InvalidArgs(format!("unknown node type `{kind}`")))?;
	let mut props = Props::for_kind(behaviour);

	if let Some(given) = args.get("props").and_then(Json::as_object) {
		for (name, value) in given {
			let key =
				PropKey::parse(name).ok_or_else(|| CanvasError::UnknownField(name.clone()))?;
			props.set(Prop::from_json(&key, value)?);
		}
	}

	let (node, _) = state.edit(false, |transaction| {
		transaction.insert(parent, None, &kind, props)
	})?;
	state.selection_mut().set([node]);
	Ok(Outcome::Redraw)
}

fn node_list(args: &Json, key: &str) -> Result<Option<Vec<NodeId>>, CanvasError> {
	let Some(list) = args.get(key) else {
		return Ok(None);
	};
	let items = list
		.as_array()
		.ok_or_else(|| CanvasError::InvalidArgs(format!("`{key}` must be a list of node ids")))?;

	items
		.iter()
		.map(|item| {
			item.as_str()
				.and_then(|id| id.parse().ok())
				.ok_or_else(|| CanvasError::InvalidArgs(format!("`{item}` is not a node id")))
		})
		.collect::<Result<Vec<_>, _>>()
		.map(Some)
}
