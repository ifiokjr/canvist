//! Text frame layout: each text node's canvist document laid out by
//! `canvist_layout` at the frame's width.

use std::sync::Arc;

use canvist_layout::DisplayItem;
use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::LayoutError;
use canvist_layout::LayoutOptions;
use canvist_layout::TextMeasurer;
use canvist_layout::Theme;
use canvist_model::Document;
use canvist_scene::NodeId;
use canvist_scene::Scene;
use canvist_scene::Shape;
use canvist_scene::TextSizing;
use kurbo::Vec2;
use rustc_hash::FxHashMap;

/// How wide auto-width text may grow before it wraps anyway.
const AUTO_WIDTH_LIMIT: f32 = 100_000.0;

/// One laid-out text frame.
#[derive(Debug)]
pub struct TextFrameLayout {
	/// The document's layout.
	pub layout: DocumentLayout,
	/// The size the frame takes: its own size, or the text's extent for
	/// auto-sized frames.
	pub size: Vec2,
	/// The display list for the whole frame, in frame coordinates.
	pub display: DisplayList,
}

/// Layouts of every text frame in a scene.
///
/// The host supplies a [`TextMeasurer`]. Painting text with the same
/// shaping (the `parley` measurer on every platform) keeps wrapping
/// identical across the canvas, PNG export, and server thumbnails.
pub struct TextLayouts {
	theme: Arc<Theme>,
	measurer: Box<dyn TextMeasurer>,
	frames: FxHashMap<NodeId, TextFrameLayout>,
}

impl std::fmt::Debug for TextLayouts {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("TextLayouts")
			.field("frames", &self.frames.len())
			.finish_non_exhaustive()
	}
}

impl TextLayouts {
	/// Layouts using `theme` and `measurer`.
	pub fn new(theme: Arc<Theme>, measurer: impl TextMeasurer + 'static) -> Self {
		Self {
			theme,
			measurer: Box::new(measurer),
			frames: FxHashMap::default(),
		}
	}

	/// The layout of text frame `id`, if it has been laid out.
	#[must_use]
	pub fn get(&self, id: NodeId) -> Option<&TextFrameLayout> {
		self.frames.get(&id)
	}

	/// Forget a frame that left the scene.
	pub fn remove(&mut self, id: NodeId) {
		self.frames.remove(&id);
	}

	/// Lay out text frame `id` from the scene, returning the size it takes,
	/// or `None` for nodes that are not text frames. A frame whose document
	/// is lent keeps its last layout (see [`TextLayouts::update`]).
	pub fn ensure(&mut self, scene: &Scene, id: NodeId) -> Result<Option<Vec2>, LayoutError> {
		let Some(node) = scene.node(id) else {
			return Ok(None);
		};
		let Shape::Text(sizing) = node.props().shape else {
			return Ok(None);
		};
		let size = node.props().size;

		match scene.text(id) {
			Some(doc) => self.lay_out(id, doc, sizing, size).map(Some),
			None => Ok(self.frames.get(&id).map(|frame| frame.size)),
		}
	}

	/// Lay out frame `id` from a document held elsewhere, such as an
	/// editor's while the frame is being edited.
	pub fn update(
		&mut self,
		scene: &Scene,
		id: NodeId,
		doc: &Document,
	) -> Result<Option<Vec2>, LayoutError> {
		let Some(node) = scene.node(id) else {
			return Ok(None);
		};
		let Shape::Text(sizing) = node.props().shape else {
			return Ok(None);
		};

		self.lay_out(id, doc, sizing, node.props().size).map(Some)
	}

	fn lay_out(
		&mut self,
		id: NodeId,
		doc: &Document,
		sizing: TextSizing,
		size: Vec2,
	) -> Result<Vec2, LayoutError> {
		let width = match sizing {
			TextSizing::AutoWidth => AUTO_WIDTH_LIMIT,
			TextSizing::AutoHeight | TextSizing::Fixed => size.x.max(1.0) as f32,
		};

		let mut layout = DocumentLayout::new(doc, self.theme.clone(), LayoutOptions::new(width));
		layout.layout_all(doc, &mut *self.measurer)?;
		let height = layout.height();
		let display = layout.display_list(&DisplayOptions::viewport(0.0, height));

		let size = match sizing {
			TextSizing::Fixed => size,
			TextSizing::AutoHeight => Vec2::new(size.x, f64::from(height)),
			TextSizing::AutoWidth => Vec2::new(f64::from(extent(&display)), f64::from(height)),
		};

		self.frames.insert(
			id,
			TextFrameLayout {
				layout,
				size,
				display,
			},
		);
		Ok(size)
	}
}

/// The right edge of the widest text run.
fn extent(display: &DisplayList) -> f32 {
	display
		.items
		.iter()
		.filter_map(|item| {
			match item {
				DisplayItem::Text(run) => Some(run.x + run.width),
				_ => None,
			}
		})
		.fold(0.0, f32::max)
}
