//! A reel: the frames of one scene, each painted by `to_svg` from a real
//! display list.
//!
//! Every top-level block is painted into its own SVG layer and positioned
//! at the rectangle layout gave it, so the video can move blocks between
//! frames along their real positions. Identical layers are stored once.

use std::collections::HashMap;

use canvist_layout::Caret;
use canvist_layout::DisplayItem;
use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::Rect;
use canvist_layout::to_svg;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Selection;
use serde_json::Value as Json;
use serde_json::json;

/// What to paint over the document in one frame, besides its blocks.
#[derive(Debug, Default)]
pub struct Overlay {
	/// The local selection, painted by layout when it is not collapsed.
	pub selection: Option<Selection>,
	/// Microseconds the change behind this frame took, if it was timed.
	pub micros: Option<f64>,
	/// A short label for the change, such as the input rule that fired.
	pub note: Option<String>,
	/// Collaborator carets as `(name, colour, selection)`.
	pub peers: Vec<(String, String, Selection)>,
	/// Extra fields for the scene.
	pub extra: Option<Json>,
}

/// The frames of one scene and the SVG layers they share.
#[derive(Debug)]
pub struct Reel {
	width: f32,
	svgs: Vec<String>,
	index: HashMap<String, usize>,
	frames: Vec<Json>,
}

impl Reel {
	/// An empty reel for a page `width` pixels wide.
	pub fn new(width: f32) -> Self {
		Self {
			width,
			svgs: Vec::new(),
			index: HashMap::new(),
			frames: Vec::new(),
		}
	}

	/// Capture `doc` as laid out by `layout`, with the caret at the head of
	/// `selection`.
	pub fn capture(
		&mut self,
		doc: &Document,
		layout: &DocumentLayout,
		selection: Option<Selection>,
		overlay: &Overlay,
	) {
		let list = display_list(layout, overlay.selection);
		self.capture_list(doc, layout, &list, selection, overlay);
	}

	/// Capture an already built display list, so the time spent building it
	/// can be measured by the caller.
	pub fn capture_list(
		&mut self,
		doc: &Document,
		layout: &DocumentLayout,
		list: &DisplayList,
		selection: Option<Selection>,
		overlay: &Overlay,
	) {
		let (blocks, chrome) = split_layers(doc, layout, list);
		let blocks: Vec<Json> = blocks
			.into_iter()
			.map(|(id, layer)| {
				let bounds = layer.bounds;
				let svg = self.intern(to_svg(&layer));
				json!([
					id.to_string(),
					svg,
					round(bounds.x),
					round(bounds.y),
					round(bounds.width),
					round(bounds.height)
				])
			})
			.collect();
		let selection_layer = chrome.map(|layer| {
			let bounds = layer.bounds;
			json!([
				self.intern(to_svg(&layer)),
				round(bounds.x),
				round(bounds.y)
			])
		});
		let caret = selection.and_then(|selection| caret_rect(layout, head_of(selection)?));
		let peers: Vec<Json> = overlay
			.peers
			.iter()
			.filter_map(|(name, color, selection)| {
				let rect = caret_rect(layout, head_of(*selection)?)?;
				Some(json!({ "name": name, "color": color, "caret": rect }))
			})
			.collect();

		self.frames.push(json!({
			"blocks": blocks,
			"selection": selection_layer,
			"caret": caret,
			"peers": peers,
			"micros": overlay.micros.map(|micros| (micros * 100.0).round() / 100.0),
			"note": overlay.note,
			"extra": overlay.extra,
			"height": round(layout.height()),
		}));
	}

	/// How many frames record an edit of `kind`, such as `InputRule`.
	pub fn count_kind(&self, kind: &str) -> usize {
		self.frames
			.iter()
			.filter(|frame| frame["extra"]["kind"].as_str() == Some(kind))
			.count()
	}

	/// The reel as JSON, with `meta` describing the scene.
	pub fn into_json(self, meta: &Json) -> Json {
		json!({
			"meta": meta,
			"width": self.width,
			"svgs": self.svgs,
			"frames": self.frames,
		})
	}

	fn intern(&mut self, svg: String) -> usize {
		if let Some(&index) = self.index.get(&svg) {
			return index;
		}

		let index = self.svgs.len();
		self.index.insert(svg.clone(), index);
		self.svgs.push(svg);
		index
	}
}

/// The display list for the whole laid-out document.
pub fn display_list(layout: &DocumentLayout, selection: Option<Selection>) -> DisplayList {
	let mut options = DisplayOptions::viewport(0.0, layout.height().max(1.0));
	options.selection = selection.filter(|selection| !selection.is_caret());
	layout.display_list(&options)
}

fn caret_rect(layout: &DocumentLayout, point: canvist_model::Point) -> Option<Json> {
	let rect = layout.caret_rect(Caret::new(point))?;
	Some(json!([
		round(rect.x),
		round(rect.y),
		round(rect.width),
		round(rect.height)
	]))
}

/// Split `list` into one display list per top-level block, in document
/// order, plus one for items that belong to no block (the selection).
fn split_layers(
	doc: &Document,
	layout: &DocumentLayout,
	list: &DisplayList,
) -> (Vec<(BlockId, DisplayList)>, Option<DisplayList>) {
	let mut groups: HashMap<BlockId, Vec<DisplayItem>> = HashMap::new();
	let mut chrome = Vec::new();

	for item in &list.items {
		let group = match item.block() {
			Some(block) => groups.entry(root_of(doc, block)).or_default(),
			None => &mut chrome,
		};

		group.push(item.clone());
	}

	let layer = |items: Vec<DisplayItem>, start: Option<Rect>| {
		let bounds = items
			.iter()
			.map(DisplayItem::bounds)
			.fold(start, |bounds, rect| {
				Some(bounds.map_or(rect, |bounds| bounds.union(&rect)))
			})
			.unwrap_or_default();
		DisplayList {
			fonts: list.fonts.clone(),
			items,
			bounds,
		}
	};
	let blocks = doc
		.children(None)
		.iter()
		.filter_map(|&root| {
			let items = groups.remove(&root)?;
			Some((root, layer(items, layout.block_rect(root))))
		})
		.collect();
	let chrome = (!chrome.is_empty()).then(|| layer(chrome, None));

	(blocks, chrome)
}

fn root_of(doc: &Document, mut block: BlockId) -> BlockId {
	while let Some(parent) = doc.block(block).and_then(canvist_model::Block::parent) {
		block = parent;
	}

	block
}

/// Round to hundredths of a pixel, which keeps the JSON small.
pub fn round(value: f32) -> f64 {
	(f64::from(value) * 100.0).round() / 100.0
}

/// The head of a text selection, where its caret is drawn.
pub fn head_of(selection: Selection) -> Option<canvist_model::Point> {
	match selection {
		Selection::Text { head, .. } => Some(head),
		Selection::Blocks { .. } => None,
	}
}
