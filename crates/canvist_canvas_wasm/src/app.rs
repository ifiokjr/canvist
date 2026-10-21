//! The canvas app without the browser: the canvas, its collaboration
//! binding, and the open text session, wired together. Everything here
//! runs natively, so it is tested without a browser.

use std::sync::Arc;
use std::time::Duration;

use canvist_canvas::Canvas;
use canvist_canvas::CanvasError;
use canvist_canvas::DesignKit;
use canvist_canvas::Input;
use canvist_canvas::KeyEvent;
use canvist_canvas::Outcome;
use canvist_canvas::Phase;
use canvist_canvas::PointerEvent;
use canvist_canvas::TextLayouts;
use canvist_canvas::TextSession;
use canvist_collab::CollabError;
use canvist_collab::Encoding;
use canvist_collab::LentText;
use canvist_collab::SceneBinding;
use canvist_collab::Track;
use canvist_editor::Editor;
use canvist_editor::EditorBuilder;
use canvist_editor::EditorError;
use canvist_editor::Env;
use canvist_editor::Update;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Theme;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_paint::Assets;
use canvist_paint::ChromeStyle;
use canvist_paint::Painter;
use canvist_paint::Quality;
use canvist_paint::Sources;
use canvist_paint::View;
use canvist_scene::Color;
use canvist_scene::NodeId;
use canvist_scene::Paint;
use canvist_scene::Props;
use canvist_scene::SceneError;
use kurbo::Affine;
use kurbo::Size;
use kurbo::Vec2;

use crate::demo;

/// Rectangles per row in [`App::add_stress`]'s grid.
const STRESS_COLUMNS: usize = 50;
/// The side of each stress rectangle, in world units.
const STRESS_SIDE: f64 = 24.0;
/// The distance between neighbouring stress rectangles' origins.
const STRESS_PITCH: f64 = 32.0;
/// Where the stress grid starts, below the demo scene.
const STRESS_ORIGIN: Vec2 = Vec2::new(80.0, 760.0);
/// Each stress rectangle's corner radius.
const STRESS_RADIUS: f64 = 4.0;
/// How far each stress rectangle's red channel steps from the last: a prime,
/// so neighbours differ and the ramp takes a while to repeat.
const STRESS_HUE_STEP: usize = 7;
/// The stress rectangles' fixed green channel.
const STRESS_GREEN: u8 = 90;

/// Why an app operation failed.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
	/// The canvas rejected an input or edit.
	#[error(transparent)]
	Canvas(#[from] CanvasError),
	/// The collaboration binding rejected an update.
	#[error(transparent)]
	Collab(#[from] CollabError),
	/// The text editor rejected an edit.
	#[error(transparent)]
	Editor(#[from] EditorError),
	/// The canvas extensions produced an invalid scene schema.
	#[error(transparent)]
	Schema(#[from] canvist_scene::SceneSchemaError),
	/// The scene rejected an edit.
	#[error(transparent)]
	Scene(#[from] SceneError),
	/// The starting scene is not valid JSON.
	#[error(transparent)]
	Json(#[from] serde_json::Error),
	/// The client id every starting scene is built under is reserved.
	#[error("client id {0} is reserved for building the starting scene")]
	ReservedClient(u32),
}

/// The canvas app: a canvas over a shared scene, and the text session
/// editing one of its frames, if any.
pub struct App {
	canvas: Canvas,
	binding: SceneBinding,
	session: Option<TextSession>,
	assets: Assets,
	/// The time of the latest input, for the text editor's undo grouping.
	now: Duration,
}

impl std::fmt::Debug for App {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("App")
			.field("canvas", &self.canvas)
			.field("session", &self.session)
			.finish_non_exhaustive()
	}
}

/// The text editor every text frame uses: the `StarterKit`. The scene's
/// text schema comes from the same builder, so the two always agree.
#[must_use]
pub fn text_editor() -> EditorBuilder {
	let mut builder = Editor::builder();
	canvist_extensions::StarterKit::add_all(&mut builder);
	builder
}

/// Text layouts measured with parley and the bundled font, which the
/// painters shape with too.
#[must_use]
pub fn text_layouts() -> TextLayouts {
	let mut measurer = ParleyMeasurer::new();
	measurer.register_font(canvist_paint::BUNDLED_FONT.to_vec());
	TextLayouts::new(Arc::new(Theme::starter_kit()), measurer)
}

impl App {
	/// The demo scene, as peer `client`. Every peer starts from the same
	/// shared state, so peers sync by exchanging what each other lacks.
	pub fn new(client: u32) -> Result<Self, AppError> {
		let scene = serde_json::from_str(demo::DEMO_SCENE)?;
		let mut app = Self::with_scene(client, &scene)?;
		app.assets = demo::assets();

		Ok(app)
	}

	/// `scene` (scene JSON, nested or flat), as peer `client`. Peers that
	/// start from the same JSON start from the same shared state, as demo
	/// peers do.
	pub fn with_scene(client: u32, scene: &serde_json::Value) -> Result<Self, AppError> {
		if client == demo::SEED_CLIENT {
			return Err(AppError::ReservedClient(client));
		}

		let mut builder = Canvas::builder().text_schema(text_editor().schema()?);
		DesignKit::register(&mut builder);
		let schema = builder.schema()?;
		let mut binding = SceneBinding::with_client_id(schema.clone(), u64::from(client))?;
		binding.apply_state(&demo::seed_state(&schema, scene)?, Encoding::V1)?;
		let scene = binding.scene(client)?;

		Ok(Self {
			canvas: builder.build(scene, text_layouts())?,
			binding,
			session: None,
			assets: Assets::new(),
			now: Duration::ZERO,
		})
	}

	/// The whole shared scene as one v1 Yjs update, to store or to give a
	/// joining peer.
	#[must_use]
	pub fn encode_state(&self) -> Vec<u8> {
		self.binding.encode_state(Encoding::V1)
	}

	/// The canvas.
	#[must_use]
	pub fn canvas(&self) -> &Canvas {
		&self.canvas
	}

	/// The open text session, if any.
	#[must_use]
	pub fn session(&self) -> Option<&TextSession> {
		self.session.as_ref()
	}

	/// Set the viewport size in CSS pixels.
	pub fn resize(&mut self, size: Size) {
		self.canvas.state_mut().set_viewport(size);
	}

	/// Switch to the tool named `name`. Returns whether one exists.
	pub fn set_tool(&mut self, name: &str) -> Result<bool, AppError> {
		let Some(tool) = self.canvas.tool_named(name) else {
			return Ok(false);
		};

		self.close_session()?;
		self.canvas.set_tool(tool);
		Ok(true)
	}

	/// Handle one input event at `now`. Returns whether to repaint.
	pub fn input(&mut self, input: &Input, now: Duration) -> Result<bool, AppError> {
		self.now = now;

		if let Some(redraw) = self.session_input(input)? {
			return Ok(redraw);
		}

		if let Input::Key(key) = input
			&& is_undo(key)
		{
			return self.undo(key.modifiers.shift);
		}

		let outcome = self.canvas.input(input)?;
		self.sync_local()?;

		if let Outcome::EditText(node) = outcome {
			self.open_session(node)?;
		}

		Ok(outcome != Outcome::Idle)
	}

	/// Type `text` into the open text session, as an IME commit or a paste.
	pub fn type_text(&mut self, text: &str, now: Duration) -> Result<bool, AppError> {
		self.now = now;
		let Some(session) = self.session.as_mut() else {
			return Ok(false);
		};
		let update = session.type_text(self.canvas.state_mut(), text, now)?;
		self.mirror_text(update)?;
		Ok(true)
	}

	/// Advance camera momentum by `elapsed` seconds. Returns whether to
	/// paint another frame.
	pub fn frame(&mut self, elapsed: f64) -> bool {
		self.canvas.frame(elapsed)
	}

	/// Integrate a collaborator's update. Returns whether anything changed.
	pub fn apply_update(&mut self, update: &[u8]) -> Result<bool, AppError> {
		let state = self.canvas.state_mut();

		let Some(session) = self.session.as_mut() else {
			let commit =
				self.binding
					.apply_update(state.scene_mut(), update, Encoding::V1, None)?;
			state.apply_remote(&commit)?;
			return Ok(!commit.is_empty());
		};

		// The open frame's text changes go to its editor.
		let mut lent = EditorLent {
			session,
			now: self.now,
			failure: None,
		};
		let applied =
			self.binding
				.apply_update(state.scene_mut(), update, Encoding::V1, Some(&mut lent));
		let commit = match (lent.failure, applied) {
			(Some(failure), _) => return Err(failure.into()),
			(None, applied) => applied?,
		};

		session.refresh(state)?;
		state.apply_remote(&commit)?;
		Ok(!commit.is_empty())
	}

	/// The v1 updates made since the last call, to broadcast.
	pub fn take_updates(&mut self) -> Vec<Vec<u8>> {
		self.binding.take_updates()
	}

	/// The shared document's v1 state vector, to ask a peer for what this
	/// peer lacks.
	#[must_use]
	pub fn state_vector(&self) -> Vec<u8> {
		self.binding.state_vector()
	}

	/// Everything the peer with v1 state vector `state_vector` lacks.
	pub fn encode_diff(&self, state_vector: &[u8]) -> Result<Vec<u8>, AppError> {
		Ok(self.binding.encode_diff(state_vector, Encoding::V1)?)
	}

	/// Add `count` rectangles in a grid below the demo, as one change: a
	/// heavier scene for measuring frame times.
	pub fn add_stress(&mut self, count: usize) -> Result<(), AppError> {
		let state = self.canvas.state_mut();
		let page = state
			.scene()
			.pages()
			.first()
			.copied()
			.unwrap_or(NodeId::ROOT);

		state.edit(false, |transaction| {
			for index in 0..count {
				transaction.insert(page, None, "rectangle", stress_props(index))?;
			}

			Ok(())
		})?;
		self.sync_local()
	}

	/// Paint the scene and its chrome, `scale` device pixels per CSS pixel.
	pub fn paint<P: Painter + ?Sized>(&self, painter: &mut P, scale: f64, quality: Quality) {
		let state = self.canvas.state();
		let Some(&page) = state.scene().pages().first() else {
			return;
		};
		let sources = Sources {
			scene: state.scene(),
			index: state.index(),
			texts: state.texts(),
			assets: &self.assets,
		};
		let view = View {
			camera: *state.camera(),
			viewport: state.viewport(),
			quality,
		};
		let mut overlay = self.canvas.overlay();

		if let Some(session) = &self.session {
			overlay.extend(session.overlay(state));
		}

		let mut scaled = canvist_paint::Transformed::new(painter, Affine::scale(scale));
		canvist_paint::paint_page(&mut scaled, &sources, page, &view);
		canvist_paint::paint_overlay(&mut scaled, &overlay, &ChromeStyle::default());
	}

	/// Send `input` to the open text session. Returns `None` when the
	/// canvas should handle it instead.
	fn session_input(&mut self, input: &Input) -> Result<Option<bool>, AppError> {
		if self.session.is_none() {
			return Ok(None);
		}

		match input {
			Input::Key(key) => self.session_key(key).map(Some),
			Input::Pointer(pointer) => self.session_pointer(pointer),
			Input::Wheel(_) | Input::Custom { .. } => Ok(None),
		}
	}

	/// A key while a text frame is open: Escape closes it, and everything
	/// else goes to the editor.
	fn session_key(&mut self, key: &KeyEvent) -> Result<bool, AppError> {
		if !key.down {
			return Ok(false);
		}

		if key.key == "Escape" {
			self.close_session()?;
			return Ok(true);
		}

		let (Some(session), state) = (self.session.as_mut(), self.canvas.state_mut()) else {
			return Ok(false);
		};
		let update = if is_character(key) {
			session.type_text(state, &key.key, self.now)?
		} else {
			session.press(state, &key.key, key.modifiers, self.now)?
		};

		self.mirror_text(update)?;
		Ok(true)
	}

	/// A pointer event while a text frame is open: a press inside it moves
	/// the caret, and a press outside ends editing and goes on to the canvas,
	/// so it can select whatever is there.
	fn session_pointer(&mut self, pointer: &PointerEvent) -> Result<Option<bool>, AppError> {
		if pointer.phase != Phase::Down {
			return Ok(Some(false));
		}

		let (Some(session), state) = (self.session.as_mut(), self.canvas.state_mut()) else {
			return Ok(None);
		};

		if !session.contains(state, pointer.position) {
			self.close_session()?;
			return Ok(None);
		}

		let update = session.click(state, pointer.position, pointer.modifiers.shift, self.now)?;
		self.mirror_text(update)?;
		Ok(Some(true))
	}

	fn open_session(&mut self, node: NodeId) -> Result<(), AppError> {
		self.close_session()?;
		self.session = Some(TextSession::open(
			self.canvas.state_mut(),
			node,
			text_editor(),
		)?);
		Ok(())
	}

	fn close_session(&mut self) -> Result<(), AppError> {
		if let Some(session) = self.session.take() {
			session.close(self.canvas.state_mut())?;
		}

		Ok(())
	}

	/// Mirror the canvas's local commits into the shared document.
	fn sync_local(&mut self) -> Result<(), AppError> {
		let state = self.canvas.state_mut();

		for local in state.take_commits() {
			let track = undo_track(local.merge);
			let follow_up = self
				.binding
				.apply_local(state.scene_mut(), &local.commit, track)?;
			state.apply_remote(&follow_up)?;
		}

		Ok(())
	}

	/// Mirror a text session's edit into the shared document.
	fn mirror_text(&mut self, update: Option<Update>) -> Result<(), AppError> {
		let (Some(update), Some(session)) = (update, self.session.as_mut()) else {
			return Ok(());
		};

		if !update.doc_changed() {
			return Ok(());
		}

		let node = session.node();
		let binding = &mut self.binding;
		session
			.editor_mut()
			.apply_external(&mut Env::headless(self.now), |doc| {
				binding
					.apply_local_text(node, doc, &update.commit, Track::Step)
					.map_err(AppError::from)
			})?;
		session.refresh(self.canvas.state_mut())?;
		Ok(())
	}

	/// Undo (or redo) this peer's own changes through the shared history,
	/// which never reverts a collaborator's work.
	fn undo(&mut self, redo: bool) -> Result<bool, AppError> {
		let state = self.canvas.state_mut();
		let commit = if redo {
			self.binding.redo(state.scene_mut(), None)?
		} else {
			self.binding.undo(state.scene_mut(), None)?
		};

		let Some(commit) = commit else {
			return Ok(false);
		};

		state.apply_remote(&commit)?;
		Ok(true)
	}
}

/// The open text session, lent to the binding while a collaborator's update
/// arrives.
struct EditorLent<'a> {
	session: &'a mut TextSession,
	now: Duration,
	/// An editor failure, which the binding's error type cannot carry.
	failure: Option<EditorError>,
}

/// What can go wrong applying a collaborator's text change in the editor.
enum LentError {
	Collab(CollabError),
	Editor(EditorError),
}

impl From<EditorError> for LentError {
	fn from(error: EditorError) -> Self {
		Self::Editor(error)
	}
}

impl LentText for EditorLent<'_> {
	fn node(&self) -> NodeId {
		self.session.node()
	}

	fn apply(
		&mut self,
		change: &mut dyn FnMut(&mut Document) -> Result<Commit, CollabError>,
	) -> Result<Commit, CollabError> {
		let result = self
			.session
			.editor_mut()
			.apply_external(&mut Env::headless(self.now), |doc| {
				change(doc).map_err(LentError::Collab)
			});

		match result {
			Ok(update) => Ok(update.commit),
			Err(LentError::Collab(error)) => Err(error),
			Err(LentError::Editor(error)) => {
				self.failure = Some(error);
				Err(CollabError::Diverged)
			}
		}
	}
}

/// Whether `key` undoes or redoes: the primary modifier with Z.
fn is_undo(key: &KeyEvent) -> bool {
	key.down && key.modifiers.primary && key.key.eq_ignore_ascii_case("z")
}

/// Whether `key` types a character rather than running a command.
fn is_character(key: &KeyEvent) -> bool {
	!key.modifiers.primary && key.key.chars().count() == 1
}

/// The props of stress rectangle `index`.
fn stress_props(index: usize) -> Props {
	let (row, column) = (index / STRESS_COLUMNS, index % STRESS_COLUMNS);
	// Grid positions are small, so they convert to `f64` exactly.
	#[allow(clippy::cast_precision_loss)]
	let origin = STRESS_ORIGIN + Vec2::new(column as f64, row as f64) * STRESS_PITCH;
	// The remainder is below `u8::MAX + 1`, so it fits.
	#[allow(clippy::cast_possible_truncation)]
	let shade = (index * STRESS_HUE_STEP % (usize::from(u8::MAX) + 1)) as u8;

	Props {
		transform: Affine::translate(origin),
		size: Vec2::new(STRESS_SIDE, STRESS_SIDE),
		fills: Arc::new([Paint::solid(Color::rgb(
			shade,
			STRESS_GREEN,
			u8::MAX - shade,
		))]),
		radius: [STRESS_RADIUS; 4],
		..Props::default()
	}
}

/// The undo track for a canvas commit: a drag's later moves join its first.
fn undo_track(merge: bool) -> Track {
	if merge { Track::Merge } else { Track::Step }
}
