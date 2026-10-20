//! Scenes: canvist's canvas documents of shapes, frames, images, vector
//! paths, and text frames, without a UI.

use std::sync::Arc;

use canvist_canvas::Canvas;
use canvist_canvas::CanvasBuilder;
use canvist_canvas::CanvasError;
use canvist_canvas::DesignKit;
use canvist_canvas::TextLayouts;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Theme;
use canvist_model::Schema;
use canvist_paint::Assets;
use canvist_paint::CpuPainter;
use canvist_paint::Draw;
use canvist_paint::Painter;
use canvist_paint::Sources;
use canvist_paint::SvgPainter;
use canvist_scene::NodeId;
use canvist_scene::Scene;
use canvist_scene::SceneError;
use canvist_scene::SceneSchema;
use canvist_scene::Transaction;
use kurbo::Affine;
use kurbo::Rect;
use kurbo::Shape as _;
use kurbo::Size;
use serde_json::Value as Json;

use crate::HeadlessError;

/// Flattening tolerance for the exported page background.
const TOLERANCE: f64 = 0.1;

/// Configures the canvas extensions a [`SceneEngine`] registers.
type Configure = dyn Fn(&mut CanvasBuilder) + Send + Sync;

/// A configured set of canvas extensions that creates [`SceneDoc`]s.
///
/// Text frames hold StarterKit documents. Build one engine per process and
/// share it: it is `Send + Sync`.
///
/// ```
/// use canvist_headless::ExportOptions;
/// use canvist_headless::SceneEngine;
/// use serde_json::json;
///
/// let engine = SceneEngine::design_kit();
/// let mut doc = engine.create()?;
/// doc.run("insert", &json!({ "type": "rectangle", "props": { "size": [120, 80] } }))?;
///
/// let svg = doc.export_svg(&ExportOptions::default())?;
/// assert!(svg.starts_with("<svg"));
/// # Ok::<(), canvist_headless::HeadlessError>(())
/// ```
pub struct SceneEngine {
	configure: Arc<Configure>,
	text: Arc<Schema>,
	schema: Arc<SceneSchema>,
	metadata: Json,
	client: u32,
}

impl std::fmt::Debug for SceneEngine {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SceneEngine")
			.field("client", &self.client)
			.finish_non_exhaustive()
	}
}

impl SceneEngine {
	/// An engine with every built-in canvas extension: shapes, frames,
	/// groups, booleans, components, images, and text frames.
	#[must_use]
	pub fn design_kit() -> Self {
		Self::new(|builder| {
			DesignKit::register(builder);
		})
		.expect("the design kit is a valid configuration")
	}

	/// An engine whose canvas extensions `configure` registers.
	pub fn new(
		configure: impl Fn(&mut CanvasBuilder) + Send + Sync + 'static,
	) -> Result<Self, HeadlessError> {
		let text = crate::Engine::starter_kit().schema().clone();
		let mut builder = Canvas::builder().text_schema(text.clone());
		configure(&mut builder);
		let schema = builder.schema()?;
		let probe = builder
			.build(
				Scene::new(schema.clone(), 1).map_err(scene_error)?,
				text_layouts(),
			)
			.map_err(HeadlessError::from)?;

		Ok(Self {
			configure: Arc::new(configure),
			text,
			metadata: probe.metadata(),
			schema,
			client: 1,
		})
	}

	/// The same engine, allocating node ids as `client`.
	#[must_use]
	pub fn with_client(mut self, client: u32) -> Self {
		self.client = client;
		self
	}

	/// The scene schema: the node types documents may hold.
	#[must_use]
	pub fn schema(&self) -> &Arc<SceneSchema> {
		&self.schema
	}

	/// Machine-readable metadata: extensions, node types, tools, commands,
	/// and inspector sections. Bindings generate their types from it.
	#[must_use]
	pub fn metadata(&self) -> &Json {
		&self.metadata
	}

	/// A new scene with one empty page.
	pub fn create(&self) -> Result<SceneDoc, HeadlessError> {
		let scene = Scene::new(self.schema.clone(), self.client).map_err(scene_error)?;
		self.open(scene)
	}

	/// A scene from scene JSON: the canonical flat form `SceneDoc::to_json`
	/// writes, or the nested form with optional ids and keys that people and
	/// agents write.
	pub fn from_json(&self, json: &Json) -> Result<SceneDoc, HeadlessError> {
		let scene =
			Scene::from_json(self.schema.clone(), self.client, json).map_err(scene_error)?;
		self.open(scene)
	}

	/// A scene joined from a peer's Yjs state, shared so it produces
	/// updates.
	#[cfg(feature = "collab")]
	pub fn from_yjs(&self, state: &[u8]) -> Result<SceneDoc, HeadlessError> {
		let mut binding = canvist_collab::SceneBinding::with_client_id(
			self.schema.clone(),
			u64::from(self.client),
		)?;
		binding.apply_state(state, canvist_collab::Encoding::V1)?;
		let scene = binding.scene(self.client)?;
		let mut doc = self.open(scene)?;
		doc.shared = Some(binding);
		Ok(doc)
	}

	fn open(&self, scene: Scene) -> Result<SceneDoc, HeadlessError> {
		let mut builder = Canvas::builder().text_schema(self.text.clone());
		(self.configure)(&mut builder);

		Ok(SceneDoc {
			canvas: builder.build(scene, text_layouts())?,
			assets: Assets::new(),
			#[cfg(feature = "collab")]
			shared: None,
		})
	}
}

/// Text layouts measured with parley and the bundled font, which the
/// painters shape with too, so text wraps the same on the server as on the
/// canvas.
fn text_layouts() -> TextLayouts {
	let mut measurer = ParleyMeasurer::new();
	measurer.register_font(canvist_paint::BUNDLED_FONT.to_vec());
	TextLayouts::new(Arc::new(Theme::starter_kit()), measurer)
}

/// What [`SceneDoc::export_svg`] and [`SceneDoc::export_png`] draw.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
	/// The nodes to draw, with everything inside them. Empty means the first
	/// page, with its background.
	pub nodes: Vec<NodeId>,
	/// Output pixels per world unit.
	pub scale: f64,
	/// Space around the drawing, in world units.
	pub padding: f64,
}

impl Default for ExportOptions {
	fn default() -> Self {
		Self {
			nodes: Vec::new(),
			scale: 1.0,
			padding: 0.0,
		}
	}
}

/// What an export draws, resolved from [`ExportOptions`].
struct Framing {
	nodes: Vec<NodeId>,
	/// The page whose background fills the image, for page exports.
	page: Option<NodeId>,
	/// Maps world coordinates to output pixels.
	to_target: Affine,
	size: Size,
}

/// One scene with a canvas around it: run commands, edit the inspector's
/// fields, transform it in transactions, and export it as SVG or PNG.
/// A `SceneDoc` is `Send`.
pub struct SceneDoc {
	canvas: Canvas,
	assets: Assets,
	#[cfg(feature = "collab")]
	shared: Option<canvist_collab::SceneBinding>,
}

impl std::fmt::Debug for SceneDoc {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SceneDoc")
			.field("nodes", &self.scene().len())
			.finish_non_exhaustive()
	}
}

impl SceneDoc {
	/// The scene.
	#[must_use]
	pub fn scene(&self) -> &Scene {
		self.canvas.state().scene()
	}

	/// The canvas, for camera, selection, and tool access.
	#[must_use]
	pub fn canvas(&self) -> &Canvas {
		&self.canvas
	}

	/// The scene as canonical JSON.
	#[must_use]
	pub fn to_json(&self) -> Json {
		self.scene().to_json()
	}

	/// Run canvas command `name` (see [`SceneEngine::metadata`]) with JSON
	/// `args`, against the selection.
	pub fn run(&mut self, name: &str, args: &Json) -> Result<(), HeadlessError> {
		self.canvas.run(name, args)?;
		self.sync()
	}

	/// Select `nodes`, for commands and inspector fields. Ids the scene does
	/// not have are an error.
	pub fn select(&mut self, nodes: &[NodeId]) -> Result<(), HeadlessError> {
		if let Some(&missing) = nodes.iter().find(|&&id| !self.scene().contains(id)) {
			return Err(scene_error(SceneError::UnknownNode(missing)));
		}

		self.canvas
			.state_mut()
			.selection_mut()
			.set(nodes.iter().copied());
		Ok(())
	}

	/// The selected nodes.
	#[must_use]
	pub fn selection(&self) -> &[NodeId] {
		self.canvas.state().selection().nodes()
	}

	/// The inspector for the selection: sections of fields and their values.
	#[must_use]
	pub fn inspect(&self) -> Json {
		self.canvas.inspect()
	}

	/// Set inspector field `key` on every selected node.
	pub fn set_field(&mut self, key: &str, value: &Json) -> Result<(), HeadlessError> {
		self.canvas.set_field(key, value)?;
		self.sync()
	}

	/// Run `edit` in one scene transaction: one undo step, and one update
	/// for collaborators.
	pub fn edit<T>(
		&mut self,
		edit: impl FnOnce(&mut Transaction<'_>) -> Result<T, SceneError>,
	) -> Result<T, HeadlessError> {
		let (value, _) = self.canvas.state_mut().edit(false, edit)?;
		self.sync()?;
		Ok(value)
	}

	/// Undo the latest change. Returns whether there was one.
	pub fn undo(&mut self) -> Result<bool, HeadlessError> {
		let undone = self.canvas.state_mut().undo()?;
		self.sync()?;
		Ok(undone)
	}

	/// Provide the pixels of image asset `asset`, `width` by `height` RGBA8.
	/// Exports draw an image whose pixels were never provided as a grey
	/// placeholder: PNG exports draw the pixels, and SVG exports reference
	/// the image by its asset id.
	pub fn add_image(
		&mut self,
		asset: &str,
		width: u32,
		height: u32,
		rgba: Vec<u8>,
	) -> Result<(), HeadlessError> {
		let expected = u64::from(width) * u64::from(height) * 4;

		if u64::try_from(rgba.len()).ok() != Some(expected) {
			return Err(HeadlessError::InvalidArgs {
				name: "addImage".to_owned(),
				reason: format!(
					"{width}×{height} RGBA8 needs {expected} bytes, got {}",
					rgba.len()
				),
			});
		}

		self.assets.insert_rgba8(asset, width, height, rgba);
		Ok(())
	}

	/// The scene, or the nodes `options` names, as an SVG document. Text
	/// stays text, and images given pixels with [`SceneDoc::add_image`]
	/// reference their asset id as `href`, for the host to resolve.
	pub fn export_svg(&self, options: &ExportOptions) -> Result<String, HeadlessError> {
		let framing = self.framing(options)?;
		let mut painter =
			SvgPainter::new(framing.size).with_images(|image| Some(image.asset.to_owned()));
		self.paint(&mut painter, &framing);
		Ok(painter.finish())
	}

	/// The scene, or the nodes `options` names, as a PNG image.
	pub fn export_png(&self, options: &ExportOptions) -> Result<Vec<u8>, HeadlessError> {
		let framing = self.framing(options)?;
		// Sizes are rounded up to whole pixels; the rasteriser rejects any
		// too large for it.
		#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
		let (width, height) = (framing.size.width as u32, framing.size.height as u32);
		let mut painter = CpuPainter::new(width, height)?;
		painter
			.shaper_mut()
			.register_font(canvist_paint::BUNDLED_FONT.to_vec());
		self.paint(&mut painter, &framing);
		Ok(painter.render_png()?)
	}

	fn framing(&self, options: &ExportOptions) -> Result<Framing, HeadlessError> {
		let invalid = |reason: &str| {
			HeadlessError::InvalidArgs {
				name: "export".to_owned(),
				reason: reason.to_owned(),
			}
		};

		if !(options.scale.is_finite() && options.scale > 0.0) {
			return Err(invalid("`scale` must be a positive number"));
		}

		if !(options.padding.is_finite() && options.padding >= 0.0) {
			return Err(invalid("`padding` must be zero or more"));
		}

		let page = options
			.nodes
			.is_empty()
			.then(|| self.scene().pages().first().copied())
			.flatten();
		let nodes = match page {
			Some(page) => self.scene().children(page).to_vec(),
			None => options.nodes.clone(),
		};
		let bounds = self
			.bounds(&nodes)
			.ok_or_else(|| invalid("there is nothing to draw"))?
			.inflate(options.padding, options.padding);
		let size = (bounds.size() * options.scale).ceil();

		Ok(Framing {
			nodes,
			page,
			to_target: Affine::scale(options.scale) * Affine::translate(-bounds.origin().to_vec2()),
			size,
		})
	}

	/// The world bounds of `nodes` and everything inside them.
	fn bounds(&self, nodes: &[NodeId]) -> Option<Rect> {
		let index = self.canvas.state().index();
		nodes
			.iter()
			.filter_map(|&id| index.get(id))
			.map(|geometry| geometry.extent)
			.reduce(|a, b| a.union(b))
	}

	fn paint<P: Painter>(&self, painter: &mut P, framing: &Framing) {
		let state = self.canvas.state();
		let sources = Sources {
			scene: state.scene(),
			index: state.index(),
			texts: state.texts(),
			assets: &self.assets,
		};

		if let Some(page) = framing.page {
			self.background(painter, page, framing.size);
		}

		canvist_paint::paint_nodes(painter, &sources, &framing.nodes, framing.to_target);
	}

	/// Fill the whole image with `page`'s fills.
	fn background<P: Painter>(&self, painter: &mut P, page: NodeId, size: Size) {
		let Some(node) = self.scene().node(page) else {
			return;
		};
		let area = Rect::from_origin_size(kurbo::Point::ORIGIN, size).to_path(TOLERANCE);

		for paint in node.props().fills.iter().filter(|paint| paint.visible) {
			if let Some((brush, unit)) = canvist_paint::convert::brush(paint, size.to_vec2()) {
				let draw = Draw {
					path: &area,
					transform: Affine::IDENTITY,
					brush: &brush,
					brush_transform: unit,
				};
				painter.fill(&draw, canvist_paint::peniko::Fill::NonZero);
			}
		}
	}

	/// Mirror the canvas's commits into the shared document, if any.
	#[cfg_attr(
		not(feature = "collab"),
		allow(
			clippy::unnecessary_wraps,
			reason = "only a shared scene can fail to mirror a commit"
		)
	)]
	fn sync(&mut self) -> Result<(), HeadlessError> {
		let state = self.canvas.state_mut();
		let commits = state.take_commits();

		#[cfg(feature = "collab")]
		if let Some(binding) = self.shared.as_mut() {
			for local in commits {
				let follow_up = binding.apply_local(
					state.scene_mut(),
					&local.commit,
					undo_track(local.merge),
				)?;
				state.apply_remote(&follow_up)?;
			}
		}

		#[cfg(not(feature = "collab"))]
		drop(commits);

		Ok(())
	}
}

/// The undo track for a canvas commit: a drag's later moves join its first.
#[cfg(feature = "collab")]
fn undo_track(merge: bool) -> canvist_collab::Track {
	if merge {
		canvist_collab::Track::Merge
	} else {
		canvist_collab::Track::Step
	}
}

#[cfg(feature = "collab")]
impl SceneDoc {
	/// Share the scene: seed a Yjs document from it, so it produces updates
	/// other peers can join from with [`SceneEngine::from_yjs`].
	pub fn share(&mut self, client_id: u64) -> Result<(), HeadlessError> {
		if self.shared.is_some() {
			return Err(HeadlessError::AlreadyShared);
		}

		let schema = self.scene().schema().clone();
		let mut binding = canvist_collab::SceneBinding::with_client_id(schema, client_id)?;
		binding.seed(self.scene())?;
		self.shared = Some(binding);
		Ok(())
	}

	/// The whole shared scene as one v1 Yjs update.
	pub fn encode_state(&self) -> Result<Vec<u8>, HeadlessError> {
		let binding = self.shared.as_ref().ok_or(HeadlessError::NotShared)?;
		Ok(binding.encode_state(canvist_collab::Encoding::V1))
	}

	/// Merge a peer's v1 update. Returns whether the scene changed.
	pub fn apply_update(&mut self, update: &[u8]) -> Result<bool, HeadlessError> {
		let binding = self.shared.as_mut().ok_or(HeadlessError::NotShared)?;
		let state = self.canvas.state_mut();
		let commit = binding.apply_update(
			state.scene_mut(),
			update,
			canvist_collab::Encoding::V1,
			None,
		)?;
		state.apply_remote(&commit)?;
		Ok(!commit.is_empty())
	}

	/// The v1 updates this scene produced since the last call.
	pub fn take_updates(&mut self) -> Vec<Vec<u8>> {
		self.shared
			.as_mut()
			.map(canvist_collab::SceneBinding::take_updates)
			.unwrap_or_default()
	}
}

fn scene_error(error: SceneError) -> HeadlessError {
	HeadlessError::Canvas(CanvasError::Scene(error))
}
