//! Flattening the block tree into rows.
//!
//! Every text block, leaf block, and empty container becomes a *row*: the
//! unit of measurement, virtualization, and hit testing. Containers become
//! *spans* over the rows they contain, remembering how far their border box
//! extends above the first row and below the last, so their bars,
//! backgrounds, and markers can be placed from row positions alone.
//!
//! Vertical spacing follows one rule: between siblings, the larger of the
//! previous block's `space_after` and the next block's `space_before`; none
//! before a first child or after a last child. The HTML renderer applies the
//! same rule, which is what keeps server-rendered HTML aligned with the
//! canvas.

use std::sync::Arc;

use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::BlockTypeId;
use canvist_model::Content;
use canvist_model::Document;
use canvist_model::Value;
use rustc_hash::FxHashMap;

use crate::geometry::to_units;
use crate::style::StyleId;
use crate::style::Styles;
use crate::style::attr;
use crate::style::attr_px;
use crate::text::Align;
use crate::text::Direction;
use crate::text::Previous;
use crate::text::TextLayout;
use crate::theme::LeafStyle;
use crate::theme::MarkerKind;
use crate::theme::TextStyle;

/// What a row holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowKind {
	/// A text block.
	Text,
	/// A leaf block such as a divider or image.
	Leaf,
	/// A container with no children, kept so its padding still takes space.
	Empty,
}

/// A list marker attached to a list item.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Marker {
	pub(crate) kind: MarkerKind,
	/// The item's number, for ordered lists.
	pub(crate) ordinal: i64,
	/// How many marker lists enclose the item's list.
	pub(crate) depth: usize,
	/// The item's `checked` attribute, for task lists.
	pub(crate) checked: bool,
}

/// One row.
#[derive(Clone, Debug)]
pub(crate) struct Row {
	pub(crate) block: BlockId,
	/// The row's stable key into [`Flat::row_slots`], which survives splices
	/// that move the row to another index.
	pub(crate) key: u32,
	pub(crate) block_type: BlockTypeId,
	pub(crate) kind: RowKind,
	pub(crate) style: StyleId,
	pub(crate) parent_kind: Option<BlockTypeId>,
	pub(crate) parent_style: Option<StyleId>,
	/// The border box's left edge and width.
	pub(crate) x: f32,
	pub(crate) width: f32,
	/// Space between the previous row's bottom and this row's top.
	pub(crate) lead: i64,
	/// The border-box height, measured or estimated.
	pub(crate) height: i64,
	/// The innermost enclosing container span.
	pub(crate) span: Option<u32>,
	/// The block revision the layout was built from.
	pub(crate) rev: u32,
	pub(crate) direction: Direction,
	/// How the row's lines sit in its content box.
	pub(crate) align: Align,
	/// The text layout, once measured. Leaf and empty rows never need one.
	pub(crate) layout: Option<Arc<TextLayout>>,
	/// The layout from before the block's text was edited, kept so the next
	/// relayout only redoes the part around the edit.
	pub(crate) previous: Option<Previous>,
	/// The marker, when the row is itself a list item.
	pub(crate) marker: Option<Marker>,
}

impl Row {
	/// Whether the row still needs measuring.
	pub(crate) fn needs_layout(&self) -> bool {
		self.kind == RowKind::Text && self.layout.is_none()
	}

	/// The row's layout, when it was built from the current revision of
	/// `block` with `style` at `content_width`; only then is it still valid.
	fn into_current_layout(
		self,
		block: &Block,
		style: StyleId,
		content_width: f32,
	) -> Option<Arc<TextLayout>> {
		if self.rev != block.rev() || self.style != style {
			return None;
		}

		self.layout
			.filter(|layout| layout.width.to_bits() == content_width.to_bits())
	}
}

/// A container block spanning a range of rows.
#[derive(Clone, Debug)]
pub(crate) struct Span {
	pub(crate) block: BlockId,
	/// The span's stable key into [`Flat::span_slots`].
	pub(crate) key: u32,
	pub(crate) block_type: BlockTypeId,
	pub(crate) style: StyleId,
	pub(crate) x: f32,
	pub(crate) width: f32,
	pub(crate) first_row: u32,
	pub(crate) last_row: u32,
	/// Distance from the border box top down to the first row's top.
	pub(crate) top: i64,
	/// Distance from the last row's bottom down to the border box bottom.
	pub(crate) bottom: i64,
	pub(crate) parent: Option<u32>,
	pub(crate) marker: Option<Marker>,
}

/// What one root-level block contributed to the flattened document.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RootExtent {
	pub(crate) block: BlockId,
	/// How many rows and spans its subtree produced.
	pub(crate) rows: u32,
	pub(crate) spans: u32,
	pub(crate) style: StyleId,
	/// The closing padding left pending after its last row, which the next
	/// root's first row inherits as leading space.
	pub(crate) closing: i64,
}

/// Where a run of root-level blocks starts: the closing padding and
/// `space_after` owed by the root before it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Seed {
	pub(crate) pending: i64,
	pub(crate) previous_after: Option<f32>,
}

/// The flattened document.
///
/// Rows and spans are found by block through stable keys, so a structural
/// edit can splice a run of rows in and only rewrite the key slots of the
/// rows after it, without rehashing them.
#[derive(Clone, Debug, Default)]
pub(crate) struct Flat {
	pub(crate) rows: Vec<Row>,
	pub(crate) spans: Vec<Span>,
	/// Block to row key.
	pub(crate) row_of: FxHashMap<BlockId, u32>,
	/// Row key to row index.
	pub(crate) row_slots: Vec<u32>,
	/// Block to span key.
	pub(crate) span_of: FxHashMap<BlockId, u32>,
	/// Span key to span index.
	pub(crate) span_slots: Vec<u32>,
	/// One extent per root-level block, in document order.
	pub(crate) roots: Vec<RootExtent>,
	/// Space below the last row: the closing padding of the containers that
	/// end the document.
	pub(crate) tail: i64,
}

/// Flatten `doc` at `width`. Rows of `previous` whose block, revision,
/// style, and width are unchanged keep their layouts; other known blocks keep
/// their old height as an estimate, which avoids scroll jumps.
pub(crate) fn flatten(doc: &Document, styles: &mut Styles, width: f32, previous: Flat) -> Flat {
	let capacity = previous.rows.len();
	let mut by_key: Vec<Option<Row>> = Vec::new();
	by_key.resize_with(previous.row_slots.len(), || None);

	for row in previous.rows {
		let key = row.key as usize;

		if let Some(slot) = by_key.get_mut(key) {
			*slot = Some(row);
		}
	}

	let flattener = Flattener {
		doc,
		styles,
		previous_index: previous.row_of,
		previous: by_key,
		flat: Flat {
			rows: Vec::with_capacity(capacity),
			row_of: FxHashMap::with_capacity_and_hasher(capacity, rustc_hash::FxBuildHasher),
			row_slots: Vec::with_capacity(capacity),
			..Flat::default()
		},
		pending: 0,
	};

	flattener.run(doc.children(None), width, Seed::default())
}

/// Flatten only the root-level blocks `roots`, starting from `seed`.
/// Indices and keys in the result are local to it; `previous` holds the old
/// rows of the same region, whose layouts are reused where still valid.
pub(crate) fn flatten_roots(
	doc: &Document,
	styles: &mut Styles,
	region: &Region<'_>,
	previous: Vec<Row>,
) -> Flat {
	let previous_index = previous
		.iter()
		.enumerate()
		.map(|(index, row)| (row.block, index as u32))
		.collect();
	let flattener = Flattener {
		doc,
		styles,
		previous_index,
		previous: previous.into_iter().map(Some).collect(),
		flat: Flat::default(),
		pending: 0,
	};

	flattener.run(region.roots, region.width, region.seed)
}

/// A run of root-level blocks to flatten.
pub(crate) struct Region<'a> {
	pub(crate) roots: &'a [BlockId],
	pub(crate) width: f32,
	pub(crate) seed: Seed,
}

/// The fraction of the font size an average glyph is assumed to advance,
/// for estimating how many lines an unmeasured text block wraps to.
const AVERAGE_GLYPH_WIDTH_EM: f32 = 0.5;

/// Where a block's children are placed.
#[derive(Clone, Copy)]
struct Frame {
	x: f32,
	width: f32,
	span: Option<u32>,
	/// How many marker-bearing containers enclose the children.
	lists: usize,
}

/// Everything about a block's place in the tree that its row or span needs.
struct Placement {
	/// The block's own resolved style.
	style: StyleId,
	parent_kind: Option<BlockTypeId>,
	parent_style: Option<StyleId>,
	frame: Frame,
	marker: Option<Marker>,
}

struct Flattener<'a> {
	doc: &'a Document,
	styles: &'a mut Styles,
	previous: Vec<Option<Row>>,
	previous_index: FxHashMap<BlockId, u32>,
	flat: Flat,
	/// Vertical space accumulated since the last row.
	pending: i64,
}

impl Flattener<'_> {
	/// Flatten the root-level blocks `ids`, recording each one's extent.
	fn run(mut self, ids: &[BlockId], width: f32, seed: Seed) -> Flat {
		let frame = Frame {
			x: 0.0,
			width,
			span: None,
			lists: 0,
		};
		let mut previous_after = seed.previous_after;
		self.pending = seed.pending;

		for &id in ids {
			let Some(block) = self.doc.block(id) else {
				continue;
			};

			previous_after = Some(self.root(block, frame, previous_after));
		}

		self.flat.tail = self.pending;
		self.flat
	}

	/// Flatten one root-level block, returning its `space_after`.
	fn root(&mut self, block: &Block, frame: Frame, previous_after: Option<f32>) -> f32 {
		let style = self.styles.resolve(block, None, None);
		let resolved = self.styles.get(style);
		let (before, after) = (resolved.space_before, resolved.space_after);

		if let Some(previous_after) = previous_after {
			self.pending += to_units(f32::max(previous_after, before));
		}

		let (rows, spans) = (self.flat.rows.len(), self.flat.spans.len());
		let placement = Placement {
			style,
			parent_kind: None,
			parent_style: None,
			frame,
			marker: None,
		};

		self.block(block, placement);
		self.flat.roots.push(RootExtent {
			block: block.id(),
			rows: (self.flat.rows.len() - rows) as u32,
			spans: (self.flat.spans.len() - spans) as u32,
			style,
			closing: self.pending,
		});

		after
	}

	fn children(&mut self, parent: Option<&Block>, parent_style: Option<StyleId>, frame: Frame) {
		let doc = self.doc;
		let parent_kind = parent.map(Block::kind);
		let marker_kind = parent_style.and_then(|style| self.styles.get(style).marker);
		let start = parent.map_or(1, |parent| self.int_attr(parent, "start").unwrap_or(1));
		let open = parent.is_none_or(|parent| is_open(doc, parent));
		let shown = if open { usize::MAX } else { 1 };
		let mut previous_after = None;

		for (index, &id) in doc
			.children(parent.map(Block::id))
			.iter()
			.enumerate()
			.take(shown)
		{
			let Some(block) = doc.block(id) else {
				continue;
			};
			let style = self.styles.resolve(block, parent_kind, parent_style);
			let resolved = self.styles.get(style);
			let (before, after) = (resolved.space_before, resolved.space_after);

			if let Some(after) = previous_after {
				self.pending += to_units(f32::max(after, before));
			}

			let marker = marker_kind
				.filter(|&kind| kind != MarkerKind::Disclosure || index == 0)
				.map(|kind| {
					let checked = if kind == MarkerKind::Disclosure {
						open
					} else {
						self.bool_attr(block, "checked")
					};

					Marker {
						kind,
						ordinal: start + index as i64,
						depth: frame.lists.saturating_sub(1),
						checked,
					}
				});
			let placement = Placement {
				style,
				parent_kind,
				parent_style,
				frame,
				marker,
			};

			self.block(block, placement);
			previous_after = Some(after);
		}
	}

	fn block(&mut self, block: &Block, placement: Placement) {
		let kind = match block.content() {
			Content::Inline(_) => RowKind::Text,
			Content::Leaf => RowKind::Leaf,
			Content::Blocks(children) if children.is_empty() => RowKind::Empty,
			Content::Blocks(_) => {
				self.container(block, placement);
				return;
			}
		};

		self.row(block, kind, placement);
	}

	fn container(&mut self, block: &Block, placement: Placement) {
		let Placement {
			style,
			frame,
			marker,
			..
		} = placement;
		let resolved = self.styles.get(style);
		let (inset_left, inset_right) = (resolved.inset_left(), resolved.inset_right());
		let (padding_top, padding_bottom) = (resolved.padding.top, resolved.padding.bottom);
		let lists = frame.lists + usize::from(resolved.marker.is_some());
		let first_row = self.flat.rows.len() as u32;
		let pending_before = self.pending;
		let index = self.push_span(block, style, frame, marker);

		let inner = Frame {
			x: frame.x + inset_left,
			width: (frame.width - inset_left - inset_right).max(0.0),
			span: Some(index),
			lists,
		};

		self.pending += to_units(padding_top);
		self.children(Some(block), Some(style), inner);
		self.pending += to_units(padding_bottom);

		let rows = &self.flat.rows;
		let top = rows
			.get(first_row as usize)
			.map_or(0, |row| row.lead - pending_before);
		let last_row = (rows.len() as u32).saturating_sub(1).max(first_row);
		let span = &mut self.flat.spans[index as usize];
		span.top = top;
		span.last_row = last_row;
		span.bottom = self.pending;
	}

	/// Open a span for container `block`, returning its index. Its row range
	/// and padding are filled in once its children are flattened.
	fn push_span(
		&mut self,
		block: &Block,
		style: StyleId,
		frame: Frame,
		marker: Option<Marker>,
	) -> u32 {
		let index = self.flat.spans.len() as u32;
		let key = self.flat.span_slots.len() as u32;
		let first_row = self.flat.rows.len() as u32;

		self.flat.span_slots.push(index);
		self.flat.span_of.insert(block.id(), key);
		self.flat.spans.push(Span {
			block: block.id(),
			key,
			block_type: block.kind(),
			style,
			x: frame.x,
			width: frame.width,
			first_row,
			last_row: first_row,
			top: 0,
			bottom: 0,
			parent: frame.span,
			marker,
		});

		index
	}

	fn row(&mut self, block: &Block, kind: RowKind, placement: Placement) {
		let content_width = self.content_width(placement.style, placement.frame.width);
		// Only text rows carry a layout worth keeping; the others' heights are
		// exact and cheap to recompute.
		let previous = self
			.take_previous(block.id())
			.filter(|_| kind == RowKind::Text);

		let (layout, height) = match previous {
			Some(old) => {
				let height = old.height;
				(
					old.into_current_layout(block, placement.style, content_width),
					height,
				)
			}
			None => (None, self.initial_height(block, kind, &placement)),
		};

		let index = self.flat.rows.len() as u32;
		let key = self.flat.row_slots.len() as u32;

		self.flat.row_slots.push(index);
		self.flat.rows.push(Row {
			block: block.id(),
			key,
			block_type: block.kind(),
			kind,
			style: placement.style,
			parent_kind: placement.parent_kind,
			parent_style: placement.parent_style,
			x: placement.frame.x,
			width: placement.frame.width,
			lead: self.pending,
			height,
			span: placement.frame.span,
			rev: block.rev(),
			direction: direction(self.doc, block),
			align: align(self.doc, block),
			layout,
			previous: None,
			marker: placement.marker,
		});
		self.flat.row_of.insert(block.id(), key);
		self.pending = 0;
	}

	/// Take the previous layout's row for `id`, so its layout can move into
	/// the new row without a reference-count bump.
	fn take_previous(&mut self, id: BlockId) -> Option<Row> {
		let index = *self.previous_index.get(&id)?;

		self.previous.get_mut(index as usize).and_then(Option::take)
	}

	fn content_width(&self, style: StyleId, width: f32) -> f32 {
		let style = self.styles.get(style);
		(width - style.inset_left() - style.inset_right()).max(0.0)
	}

	/// The exact height of leaf and empty rows, or an estimate for text.
	fn initial_height(&self, block: &Block, kind: RowKind, placement: &Placement) -> i64 {
		let resolved = self.styles.get(placement.style);
		let padding = resolved.padding.vertical();

		let content = match kind {
			RowKind::Empty => 0.0,
			RowKind::Leaf => leaf_height(block, resolved.leaf),
			RowKind::Text => {
				let width = self.content_width(placement.style, placement.frame.width);
				estimated_text_height(block, &resolved.text, width)
			}
		};

		to_units(padding + content)
	}

	fn int_attr(&self, block: &Block, key: &str) -> Option<i64> {
		let defaults = &self.doc.schema().block(block.kind()).attrs;
		attr(block.attrs(), defaults, key).and_then(Value::as_int)
	}

	fn bool_attr(&self, block: &Block, key: &str) -> bool {
		let defaults = &self.doc.schema().block(block.kind()).attrs;
		attr(block.attrs(), defaults, key)
			.and_then(Value::as_bool)
			.unwrap_or(false)
	}
}

/// Whether a container shows all its children: unless its `open`
/// attribute is `false`, when only the first one shows, as a closed
/// `<details>` shows only its summary.
pub(crate) fn is_open(doc: &Document, block: &Block) -> bool {
	let defaults = &doc.schema().block(block.kind()).attrs;
	attr(block.attrs(), defaults, "open").and_then(Value::as_bool) != Some(false)
}

/// The paragraph direction a block's `dir` attribute asks for.
pub(crate) fn direction(doc: &Document, block: &Block) -> Direction {
	let defaults = &doc.schema().block(block.kind()).attrs;

	match attr(block.attrs(), defaults, "dir").and_then(Value::as_str) {
		Some("rtl") => Direction::Rtl,
		Some("ltr") => Direction::Ltr,
		_ => Direction::Auto,
	}
}

/// The line alignment a block's `textAlign` attribute asks for.
pub(crate) fn align(doc: &Document, block: &Block) -> Align {
	let defaults = &doc.schema().block(block.kind()).attrs;
	Align::from_attr(attr(block.attrs(), defaults, "textAlign").and_then(Value::as_str))
}

/// The height of a leaf block's content: a rule's thickness, or a slot's
/// `height` attribute falling back to the theme's default.
fn leaf_height(block: &Block, leaf: LeafStyle) -> f32 {
	match leaf {
		LeafStyle::Rule { thickness, .. } => thickness,
		LeafStyle::Slot { height } => attr_px(block.attrs(), "height").unwrap_or(height),
	}
}

/// An estimate of a text block's content height at `width`, from its length
/// and an average glyph advance. Measuring replaces it once the row is near
/// the viewport.
fn estimated_text_height(block: &Block, text: &TextStyle, width: f32) -> f32 {
	let length = block.inline().map_or(0, canvist_model::InlineText::len) as f32;
	let average = text.font.size * AVERAGE_GLYPH_WIDTH_EM + text.letter_spacing;
	let lines = (length * average / width.max(1.0)).ceil().max(1.0);

	lines * text.line_height_px()
}
