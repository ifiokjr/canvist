//! The built-in extensions. An app registers the ones it uses; each adds
//! its node types, tools, commands, and inspector sections.

use std::sync::Arc;

use canvist_scene::BoolOp;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::NodeSpec;
use canvist_scene::Paint;
use canvist_scene::PaintKind;
use canvist_scene::Prop;
use canvist_scene::Props;
use canvist_scene::SceneSchemaBuilder;
use canvist_scene::Shape;
use kurbo::Affine;
use kurbo::Vec2;
use serde_json::Value as Json;

use crate::CanvasError;
use crate::CanvasState;
use crate::extension::CanvasExtension;
use crate::extension::Command;
use crate::extension::Registration;
use crate::extension::ToolId;
use crate::extension::TypedCanvasExtension;
use crate::inspector::Control;
use crate::inspector::Field;
use crate::inspector::Section;
use crate::tools::Outcome;
use crate::tools::PenTool;
use crate::tools::ShapeTool;
use crate::tools::TextTool;
use crate::tools::Tool;

fn node_type(schema: SceneSchemaBuilder, name: &str, kind: NodeKind) -> SceneSchemaBuilder {
	if schema.has(name) {
		schema
	} else {
		schema.node(NodeSpec::new(name, kind))
	}
}

/// Rectangles, ellipses, polygons and stars, and vector paths, with a tool
/// for each.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shapes;

/// The [`Shapes`] tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapesHandle {
	/// Draws rectangles.
	pub rectangle: ToolId,
	/// Draws ellipses.
	pub ellipse: ToolId,
	/// Draws polygons.
	pub polygon: ToolId,
	/// Draws vector paths.
	pub pen: ToolId,
}

impl CanvasExtension for Shapes {
	fn name(&self) -> &'static str {
		"shapes"
	}

	fn schema(&self, schema: SceneSchemaBuilder) -> SceneSchemaBuilder {
		[
			("rectangle", NodeKind::Rectangle),
			("ellipse", NodeKind::Ellipse),
			("polygon", NodeKind::Polygon),
			("path", NodeKind::Path),
		]
		.into_iter()
		.fold(schema, |schema, (name, kind)| node_type(schema, name, kind))
	}

	fn tools(&self) -> Vec<Box<dyn Tool>> {
		vec![
			Box::new(ShapeTool::new("rectangle", "rectangle")),
			Box::new(ShapeTool::new("ellipse", "ellipse")),
			Box::new(ShapeTool::new("polygon", "polygon")),
			Box::new(PenTool::new()),
		]
	}

	fn inspector(&self) -> Vec<Section> {
		vec![
			Section::new(
				"corners",
				"Corners",
				vec![Field::new("radius", "Radius", Control::Json)],
			)
			.for_kinds(&[
				NodeKind::Rectangle,
				NodeKind::Frame,
				NodeKind::Image,
				NodeKind::Component,
			]),
			Section::new(
				"shape",
				"Shape",
				vec![Field::new("shape", "Shape", Control::Json)],
			)
			.for_kinds(&[NodeKind::Ellipse, NodeKind::Polygon, NodeKind::Path]),
		]
	}
}

impl TypedCanvasExtension for Shapes {
	type Handle = ShapesHandle;

	fn handle(&self, registration: Registration) -> ShapesHandle {
		ShapesHandle {
			rectangle: registration.tool(0),
			ellipse: registration.tool(1),
			polygon: registration.tool(2),
			pen: registration.tool(3),
		}
	}
}

/// Frames, groups, booleans, components, and instances, with a frame tool
/// and the `group`, `ungroup`, `boolean`, `component`, and `instance`
/// commands.
#[derive(Clone, Copy, Debug, Default)]
pub struct Frames;

/// The [`Frames`] tool and commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramesHandle {
	/// Draws frames.
	pub frame: ToolId,
}

impl FramesHandle {
	/// Wrap the selection in a group.
	pub fn group(self, state: &mut CanvasState) -> Result<NodeId, CanvasError> {
		wrap(state, "group", Props::for_kind(NodeKind::Group))
	}

	/// Combine the selection into a boolean.
	pub fn boolean(self, state: &mut CanvasState, op: BoolOp) -> Result<NodeId, CanvasError> {
		let props = Props {
			shape: Shape::Boolean(op),
			..Props::default()
		};
		wrap(state, "boolean", props)
	}

	/// Replace each selected group or boolean with its children.
	pub fn ungroup(self, state: &mut CanvasState) -> Result<(), CanvasError> {
		ungroup(state)
	}
}

impl CanvasExtension for Frames {
	fn name(&self) -> &'static str {
		"frames"
	}

	fn schema(&self, schema: SceneSchemaBuilder) -> SceneSchemaBuilder {
		[
			("frame", NodeKind::Frame),
			("group", NodeKind::Group),
			("boolean", NodeKind::Boolean),
			("component", NodeKind::Component),
			("instance", NodeKind::Instance),
		]
		.into_iter()
		.fold(schema, |schema, (name, kind)| node_type(schema, name, kind))
	}

	fn tools(&self) -> Vec<Box<dyn Tool>> {
		vec![Box::new(ShapeTool::new("frame", "frame"))]
	}

	fn commands(&self) -> Vec<Command> {
		let ops: Vec<&'static str> = BoolOp::ALL.iter().map(|op| op.name()).collect();

		vec![
			Command::new(
				"group",
				"Wrap the selected siblings in a group.",
				Vec::new(),
				|state, _| {
					wrap(state, "group", Props::for_kind(NodeKind::Group)).map(|_| Outcome::Redraw)
				},
			),
			Command::new(
				"ungroup",
				"Replace selected groups and booleans with their children.",
				Vec::new(),
				|state, _| ungroup(state).map(|()| Outcome::Redraw),
			),
			Command::new(
				"boolean",
				"Combine the selected siblings with a boolean operation.",
				vec![Field::new("op", "Operation", Control::Select(ops))],
				|state, args| {
					let name = string_arg(args, "op")?;
					let operation = BoolOp::ALL
						.into_iter()
						.find(|candidate| candidate.name() == name)
						.ok_or_else(|| {
							CanvasError::InvalidArgs(format!("unknown boolean operation `{name}`"))
						})?;
					let props = Props {
						shape: Shape::Boolean(operation),
						..Props::default()
					};
					wrap(state, "boolean", props).map(|_| Outcome::Redraw)
				},
			),
			Command::new(
				"instance",
				"Place an instance of a component.",
				vec![
					Field::new("component", "Component", Control::Text),
					Field::new("x", "X", Control::number(1.0, "px")),
					Field::new("y", "Y", Control::number(1.0, "px")),
				],
				instance,
			),
		]
	}

	fn inspector(&self) -> Vec<Section> {
		vec![
			Section::new(
				"frame",
				"Frame",
				vec![
					Field::new("clip", "Clip content", Control::Toggle),
					Field::new("layout", "Auto layout", Control::Json),
				],
			)
			.for_kinds(&[NodeKind::Frame, NodeKind::Component]),
			Section::new(
				"constraints",
				"Constraints",
				vec![Field::new("constraints", "Constraints", Control::Json)],
			),
		]
	}
}

impl TypedCanvasExtension for Frames {
	type Handle = FramesHandle;

	fn handle(&self, registration: Registration) -> FramesHandle {
		FramesHandle {
			frame: registration.tool(0),
		}
	}
}

/// Text frames that run the canvist text editor, with a text tool. Needs a
/// text schema on the builder.
#[derive(Clone, Copy, Debug, Default)]
pub struct Texts;

/// The [`Texts`] tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextsHandle {
	/// Places text frames.
	pub text: ToolId,
}

impl CanvasExtension for Texts {
	fn name(&self) -> &'static str {
		"texts"
	}

	fn schema(&self, schema: SceneSchemaBuilder) -> SceneSchemaBuilder {
		node_type(schema, "text", NodeKind::Text)
	}

	fn tools(&self) -> Vec<Box<dyn Tool>> {
		vec![Box::new(TextTool::new())]
	}

	fn inspector(&self) -> Vec<Section> {
		let sizing: Vec<&'static str> = canvist_scene::TextSizing::ALL
			.iter()
			.map(|sizing| sizing.name())
			.collect();
		vec![
			Section::new(
				"text",
				"Text",
				vec![Field::new("shape", "Sizing", Control::Select(sizing))],
			)
			.for_kinds(&[NodeKind::Text]),
		]
	}
}

impl TypedCanvasExtension for Texts {
	type Handle = TextsHandle;

	fn handle(&self, registration: Registration) -> TextsHandle {
		TextsHandle {
			text: registration.tool(0),
		}
	}
}

/// Image nodes and the `image` command.
#[derive(Clone, Copy, Debug, Default)]
pub struct Images;

/// The [`Images`] commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImagesHandle;

impl ImagesHandle {
	/// Place image `asset` at world rectangle `x, y, width, height` on the
	/// first page.
	pub fn insert(
		self,
		state: &mut CanvasState,
		asset: &str,
		origin: Vec2,
		size: Vec2,
	) -> Result<NodeId, CanvasError> {
		insert_image(state, asset, origin, size)
	}
}

impl CanvasExtension for Images {
	fn name(&self) -> &'static str {
		"images"
	}

	fn schema(&self, schema: SceneSchemaBuilder) -> SceneSchemaBuilder {
		node_type(schema, "image", NodeKind::Image)
	}

	fn commands(&self) -> Vec<Command> {
		vec![Command::new(
			"image",
			"Place an image by asset id.",
			vec![
				Field::new("asset", "Asset", Control::Text),
				Field::new("x", "X", Control::number(1.0, "px")),
				Field::new("y", "Y", Control::number(1.0, "px")),
				Field::new("width", "Width", Control::number(1.0, "px")),
				Field::new("height", "Height", Control::number(1.0, "px")),
			],
			|state, args| {
				let origin = Vec2::new(number_arg(args, "x")?, number_arg(args, "y")?);
				let size = Vec2::new(number_arg(args, "width")?, number_arg(args, "height")?);
				insert_image(state, &string_arg(args, "asset")?, origin, size)
					.map(|_| Outcome::Redraw)
			},
		)]
	}
}

impl TypedCanvasExtension for Images {
	type Handle = ImagesHandle;

	fn handle(&self, _registration: Registration) -> ImagesHandle {
		ImagesHandle
	}
}

/// Every built-in extension.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesignKit;

/// The [`DesignKit`] handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesignKitHandle {
	/// Shapes.
	pub shapes: ShapesHandle,
	/// Frames.
	pub frames: FramesHandle,
	/// Text frames.
	pub texts: TextsHandle,
	/// Images.
	pub images: ImagesHandle,
}

impl DesignKit {
	/// Register every built-in extension on `builder`.
	pub fn register(builder: &mut crate::CanvasBuilder) -> DesignKitHandle {
		DesignKitHandle {
			shapes: builder.add(Shapes),
			frames: builder.add(Frames),
			texts: builder.add(Texts),
			images: builder.add(Images),
		}
	}
}

fn string_arg(args: &Json, key: &str) -> Result<String, CanvasError> {
	args.get(key)
		.and_then(Json::as_str)
		.map(str::to_owned)
		.ok_or_else(|| CanvasError::InvalidArgs(format!("`{key}` must be a string")))
}

fn number_arg(args: &Json, key: &str) -> Result<f64, CanvasError> {
	args.get(key)
		.and_then(Json::as_f64)
		.filter(|value| value.is_finite())
		.ok_or_else(|| CanvasError::InvalidArgs(format!("`{key}` must be a number")))
}

/// Wrap the selected siblings in a new container of type `kind`, keeping
/// their order and where they appear.
fn wrap(state: &mut CanvasState, kind: &str, props: Props) -> Result<NodeId, CanvasError> {
	let scene = state.scene();
	let mut nodes = state.selection().nodes().to_vec();
	let parent = shared_parent(state, &nodes)?;
	nodes.sort_by(|&a, &b| crate::hit::paint_order(scene, a, b));
	let top = nodes
		.iter()
		.filter_map(|&id| scene.index_in_parent(id))
		.max()
		.map_or(0, |index| index + 1);
	let kind = kind.to_owned();
	let (container, _) = state.edit(false, |transaction| {
		let container = transaction.insert(parent, Some(top), &kind, props)?;

		for &node in &nodes {
			transaction.move_to(node, container, None)?;
		}

		Ok(container)
	})?;
	state.selection_mut().set([container]);
	Ok(container)
}

fn shared_parent(state: &CanvasState, nodes: &[NodeId]) -> Result<NodeId, CanvasError> {
	let scene = state.scene();
	let mut parents = nodes
		.iter()
		.filter_map(|&id| scene.node(id))
		.map(canvist_scene::Node::parent);
	let first = parents
		.next()
		.ok_or_else(|| CanvasError::InvalidArgs("select something first".into()))?;

	if parents.all(|parent| parent == first) && !first.is_root() {
		Ok(first)
	} else {
		Err(CanvasError::InvalidArgs(
			"select siblings under one parent".into(),
		))
	}
}

/// Move each selected group's or boolean's children to its parent, then
/// remove it.
fn ungroup(state: &mut CanvasState) -> Result<(), CanvasError> {
	let scene = state.scene();
	let groups: Vec<NodeId> = state
		.selection()
		.nodes()
		.iter()
		.copied()
		.filter(|&id| matches!(scene.kind(id), Some(NodeKind::Group | NodeKind::Boolean)))
		.collect();
	let mut freed = Vec::new();

	let ((), _) = state.edit(false, |transaction| {
		for &group in &groups {
			let scene = transaction.scene();
			let Some(node) = scene.node(group) else {
				continue;
			};
			let (parent, outer) = (node.parent(), node.props().transform);
			let position = scene.index_in_parent(group).unwrap_or(0);
			let children = scene.children(group).to_vec();

			for (offset, child) in children.into_iter().enumerate() {
				let inner = transaction
					.scene()
					.node(child)
					.map_or(Affine::IDENTITY, |found| found.props().transform);
				transaction.move_to(child, parent, Some(position + offset + 1))?;
				transaction.set(child, Prop::Transform(outer * inner))?;
				freed.push(child);
			}

			transaction.remove(&[group])?;
		}

		Ok(())
	})?;
	state.selection_mut().set(freed);
	Ok(())
}

fn instance(state: &mut CanvasState, args: &Json) -> Result<Outcome, CanvasError> {
	let component: NodeId = string_arg(args, "component")?
		.parse()
		.map_err(|error| CanvasError::InvalidArgs(format!("{error}")))?;
	let size = state
		.index()
		.get(component)
		.filter(|_| state.scene().kind(component) == Some(NodeKind::Component))
		.map(|geometry| geometry.size)
		.ok_or_else(|| CanvasError::InvalidArgs(format!("{component} is not a component")))?;
	let origin = Vec2::new(number_arg(args, "x")?, number_arg(args, "y")?);
	let page = state
		.scene()
		.pages()
		.first()
		.copied()
		.unwrap_or(NodeId::ROOT);
	let props = Props {
		transform: Affine::translate(origin),
		size,
		shape: Shape::Instance { component },
		..Props::default()
	};
	let (node, _) = state.edit(false, |transaction| {
		transaction.insert(page, None, "instance", props)
	})?;
	state.selection_mut().set([node]);
	Ok(Outcome::Redraw)
}

fn insert_image(
	state: &mut CanvasState,
	asset: &str,
	origin: Vec2,
	size: Vec2,
) -> Result<NodeId, CanvasError> {
	let page = state
		.scene()
		.pages()
		.first()
		.copied()
		.unwrap_or(NodeId::ROOT);
	let props = Props {
		transform: Affine::translate(origin),
		size,
		fills: Arc::new([Paint::new(PaintKind::Image {
			asset: asset.into(),
			fit: canvist_scene::ImageFit::Fill,
		})]),
		..Props::for_kind(NodeKind::Image)
	};
	let (node, _) = state.edit(false, |transaction| {
		transaction.insert(page, None, "image", props)
	})?;
	state.selection_mut().set([node]);
	Ok(node)
}
