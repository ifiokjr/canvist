//! Document layout: rows, containers, virtualization, and incremental
//! relayout.

use std::sync::Arc;

use canvist_model::BlockId;
use canvist_model::Commit;
use canvist_model::Document;

use self::heights::Heights;
#[expect(
	unused_imports,
	reason = "the display list, the next layer of this stack, is the caller"
)]
pub(crate) use self::query::Origin;
use self::rows::Flat;
pub(crate) use self::rows::Marker;
pub(crate) use self::rows::Row;
pub(crate) use self::rows::RowKind;
pub(crate) use self::rows::Span;
use crate::BreakOverride;
use crate::LayoutError;
use crate::Rect;
use crate::geometry::to_px;
use crate::geometry::to_units;
use crate::measure::MeasureCache;
use crate::style::BlockStyle;
use crate::style::StyleId;
use crate::style::Styles;
use crate::text;
use crate::text::ItemContext;
use crate::text::Previous;
use crate::text::TextEdit;
use crate::theme::MarkerKind;
use crate::theme::Theme;

mod dump;
mod edits;
mod heights;
mod query;
mod rows;
mod splice;

/// Layout settings.
#[derive(Clone, Copy, Debug)]
pub struct LayoutOptions {
	/// The width of the document's content box in pixels.
	pub width: f32,
	/// Extra distance above and below a requested range to lay out, so
	/// scrolling a little never waits on measurement.
	pub overscan: f32,
	/// A host tailoring of UAX #14 break opportunities.
	pub break_override: Option<BreakOverride>,
}

impl LayoutOptions {
	/// Options for a document `width` pixels wide, with no overscan.
	#[must_use]
	pub const fn new(width: f32) -> Self {
		Self {
			width,
			overscan: 0.0,
			break_override: None,
		}
	}

	/// Set the overscan distance.
	#[must_use]
	pub const fn overscan(mut self, overscan: f32) -> Self {
		self.overscan = overscan;
		self
	}

	/// Set the break override.
	#[must_use]
	pub const fn break_override(mut self, break_override: BreakOverride) -> Self {
		self.break_override = Some(break_override);
		self
	}
}

/// What [`DocumentLayout::ensure`] changed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewportUpdate {
	/// How far the row at the top of the requested range moved, because
	/// blocks above it replaced their estimated heights with measured ones.
	/// Add it to the scroll position to keep the content on screen still.
	pub shift: f32,
	/// The top of the first row whose height changed, if any. Everything
	/// below it may have moved.
	pub moved_from: Option<f32>,
	/// How many blocks were laid out.
	pub laid_out: usize,
	/// How many times the measurer was called.
	pub measurer_calls: u64,
}

/// What a commit invalidated, in document coordinates before relayout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Damage {
	/// The old rectangles of changed blocks.
	pub rects: Vec<Rect>,
	/// When the document structure changed, the top of the first affected
	/// block: everything below it must be repainted.
	pub from_y: Option<f32>,
}

/// The layout of a whole document.
///
/// Text blocks are measured lazily: [`DocumentLayout::ensure`] lays out the
/// blocks in a vertical range and the rest keep estimated heights, so opening
/// a large document costs one viewport of measurement. Apply every
/// [`Commit`] with [`DocumentLayout::apply`] and only the touched blocks are
/// laid out again.
#[derive(Clone, Debug)]
pub struct DocumentLayout {
	styles: Styles,
	cache: MeasureCache,
	options: LayoutOptions,
	flat: Flat,
	heights: Heights,
}

impl DocumentLayout {
	/// Flatten `doc` with `theme`. Nothing is measured yet.
	#[must_use]
	pub fn new(doc: &Document, theme: Arc<Theme>, options: LayoutOptions) -> Self {
		let mut layout = Self {
			styles: Styles::new(theme, doc.schema().clone()),
			cache: MeasureCache::default(),
			options,
			flat: Flat::default(),
			heights: Heights::default(),
		};
		layout.rebuild(doc);
		layout
	}

	/// The theme.
	#[must_use]
	pub fn theme(&self) -> &Arc<Theme> {
		self.styles.theme()
	}

	/// The layout options.
	#[must_use]
	pub fn options(&self) -> &LayoutOptions {
		&self.options
	}

	/// The document's total height, including estimates for blocks that have
	/// not been measured.
	#[must_use]
	pub fn height(&self) -> f32 {
		to_px(self.heights.total() + self.tail())
	}

	/// How many times the measurer has been called over the layout's life.
	#[must_use]
	pub fn measurer_calls(&self) -> u64 {
		self.cache.round_trips()
	}

	/// Replace the theme. Measurements are kept; every block is laid out
	/// again on demand.
	pub fn set_theme(&mut self, doc: &Document, theme: Arc<Theme>) {
		if theme.version() == self.theme().version() {
			return;
		}

		self.styles = Styles::new(theme, doc.schema().clone());
		self.forget_layouts();
		self.rebuild(doc);
	}

	/// Drop every measurement and lay every block out again on demand,
	/// keeping each block's height as an estimate until then. Call this when
	/// a web font finishes loading: text measured with its fallback face is
	/// the wrong width. Font ids stay the same.
	pub fn forget_measurements(&mut self) {
		self.cache.forget_measurements();
		self.forget_layouts();
	}

	/// Change the content width. Every block is laid out again on demand;
	/// until then each keeps its old height as an estimate.
	pub fn set_width(&mut self, doc: &Document, width: f32) {
		if width.to_bits() == self.options.width.to_bits() {
			return;
		}

		self.options.width = width;
		self.rebuild(doc);
	}

	/// Account for a commit. Touched blocks lose their layout and are laid
	/// out again by the next [`DocumentLayout::ensure`]. Structural changes,
	/// and blocks whose style changed, re-flatten only the root-level
	/// subtrees they touched; every other block keeps its row and layout.
	pub fn apply(&mut self, doc: &Document, commit: &Commit) -> Damage {
		let mut damage = Damage::default();
		let mut anchors = Vec::new();

		for &id in &commit.touched {
			let edit = edits::text_edit(id, &commit.steps);
			self.invalidate(doc, (id, edit), &mut damage, &mut anchors);
		}

		if !commit.structural && anchors.is_empty() {
			return damage;
		}

		let top = damage.rects.iter().map(|rect| rect.y).reduce(f32::min);
		damage.from_y = Some(top.unwrap_or(0.0));

		if !self.splice(doc, commit, &anchors) {
			self.rebuild(doc);
		}

		damage
	}

	/// Drop the layout of touched block `id`, recording its old rectangle,
	/// and anchor it for re-flattening when its style or children changed.
	/// When only its text changed, by `edit`, the old layout is kept so the
	/// next relayout can patch it.
	fn invalidate(
		&mut self,
		doc: &Document,
		(id, edit): (BlockId, Option<TextEdit>),
		damage: &mut Damage,
		anchors: &mut Vec<BlockId>,
	) {
		if self.span_index(id).is_some() {
			anchors.push(id);
		}

		let Some(index) = self.row_index(id) else {
			return;
		};

		damage.rects.push(self.row_rect(index));

		let row = &mut self.flat.rows[index];
		let layout = row.layout.take();
		let previous = row.previous.take();

		row.previous = edit.and_then(|edit| {
			match (previous, layout) {
				(Some(previous), _) => Some(previous.then(edit)),
				(None, Some(layout)) => Some(Previous { layout, edit }),
				(None, None) => None,
			}
		});

		let Some(block) = doc.block(id) else {
			anchors.push(id);

			return;
		};
		let style = self
			.styles
			.resolve(block, row.parent_kind, row.parent_style);
		row.rev = block.rev();

		if style != row.style {
			anchors.push(id);
		}
	}

	/// Lay out every block intersecting `top..bottom` (widened by the
	/// overscan), measuring with at most one measurer call in the common
	/// case. A second call happens only when estimated heights were so far
	/// off that measured blocks no longer cover the range.
	pub fn ensure(
		&mut self,
		doc: &Document,
		measurer: &mut dyn crate::TextMeasurer,
		top: f32,
		bottom: f32,
	) -> Result<ViewportUpdate, LayoutError> {
		let calls_before = self.cache.round_trips();
		let anchor = self.row_at(top);
		let anchor_top = anchor.map(|index| self.row_top(index));
		let mut update = ViewportUpdate::default();
		let overscan = self.options.overscan;

		for _ in 0..4 {
			let todo: Vec<usize> = self
				.rows_between(top - overscan, bottom + overscan)
				.filter(|&index| self.flat.rows[index].needs_layout())
				.collect();

			if todo.is_empty() {
				break;
			}

			let moved = self.lay_out_rows(doc, measurer, &todo)?;
			update.laid_out += todo.len();

			update.moved_from = match (update.moved_from, moved) {
				(Some(a), Some(b)) => Some(a.min(b)),
				(a, b) => a.or(b),
			};
		}

		if let (Some(anchor), Some(before)) = (anchor, anchor_top) {
			update.shift = to_px(self.row_top(anchor) - before);
		}

		update.measurer_calls = self.cache.round_trips() - calls_before;
		Ok(update)
	}

	/// Lay out every block, for server rendering, export, and tests.
	pub fn layout_all(
		&mut self,
		doc: &Document,
		measurer: &mut dyn crate::TextMeasurer,
	) -> Result<(), LayoutError> {
		let todo: Vec<usize> = (0..self.flat.rows.len())
			.filter(|&index| self.flat.rows[index].needs_layout())
			.collect();

		if !todo.is_empty() {
			self.lay_out_rows(doc, measurer, &todo)?;
		}

		Ok(())
	}

	/// Whether every block has been measured.
	#[must_use]
	pub fn is_fully_measured(&self) -> bool {
		self.flat.rows.iter().all(|row| !row.needs_layout())
	}

	/// The border box of block `id`, for any block including containers.
	#[must_use]
	pub fn block_rect(&self, id: BlockId) -> Option<Rect> {
		if let Some(index) = self.row_index(id) {
			return Some(self.row_rect(index));
		}

		let span = self.span_index(id)?;
		Some(self.span_rect(&self.flat.spans[span]))
	}

	/// The number of laid-out lines of text block `id`, or `None` if it is
	/// not a measured text block.
	#[must_use]
	pub fn line_count(&self, id: BlockId) -> Option<usize> {
		let index = self.row_index(id)?;
		Some(self.flat.rows[index].layout.as_ref()?.lines.len())
	}

	/// Blocks with rows intersecting `top..bottom`, in document order.
	#[must_use]
	pub fn blocks_between(&self, top: f32, bottom: f32) -> Vec<BlockId> {
		self.rows_between(top, bottom)
			.map(|index| self.flat.rows[index].block)
			.collect()
	}

	fn rebuild(&mut self, doc: &Document) {
		let previous = std::mem::take(&mut self.flat);
		self.flat = rows::flatten(doc, &mut self.styles, self.options.width, previous);
		self.rebuild_heights();
	}

	fn rebuild_heights(&mut self) {
		self.heights = Heights::new(
			self.flat
				.rows
				.iter()
				.map(|row| row.lead + row.height)
				.collect(),
		);
	}

	fn forget_layouts(&mut self) {
		for row in &mut self.flat.rows {
			row.layout = None;
			row.previous = None;
		}
	}

	/// Measure and lay out `rows` with one measurer call. Returns the top of
	/// the first row whose height changed.
	fn lay_out_rows(
		&mut self,
		doc: &Document,
		measurer: &mut dyn crate::TextMeasurer,
		rows: &[usize],
	) -> Result<Option<f32>, LayoutError> {
		let drafts = self.prepare_rows(doc, rows)?;

		self.request_markers(rows);
		self.cache.flush(measurer)?;
		self.finish_rows(drafts)
	}

	/// Itemize the text of `rows`, queueing every run they need measured.
	fn prepare_rows(
		&mut self,
		doc: &Document,
		rows: &[usize],
	) -> Result<Vec<(usize, text::Draft)>, LayoutError> {
		let mut drafts = Vec::with_capacity(rows.len());
		let mut context = ItemContext {
			styles: &self.styles,
			cache: &mut self.cache,
			break_override: self.options.break_override,
		};

		for &index in rows {
			let row = &mut self.flat.rows[index];
			let inline = doc
				.block(row.block)
				.and_then(canvist_model::Block::inline)
				.ok_or(LayoutError::OutOfSync(row.block))?;
			let style = self.styles.get(row.style);
			let block = text::Block {
				base: &style.text,
				inline,
				direction: row.direction,
				width: content_width(style, row.width),
				previous: row.previous.take(),
			};

			drafts.push((index, text::prepare(&mut context, block)));
		}

		Ok(drafts)
	}

	/// Break the measured `drafts` into lines and store their layouts and
	/// heights. Returns the top of the first row whose height changed.
	fn finish_rows(
		&mut self,
		drafts: Vec<(usize, text::Draft)>,
	) -> Result<Option<f32>, LayoutError> {
		let tab_size = self.theme().tab_size();
		let mut moved = None;

		for (index, draft) in drafts {
			let style = self.styles.get(self.flat.rows[index].style);
			let width = content_width(style, self.flat.rows[index].width);
			let padding = style.padding.vertical();
			let layout = text::finish(draft, &self.cache, width, tab_size)?;
			let height = to_units(padding + layout.height);
			let row = &mut self.flat.rows[index];
			row.layout = Some(Arc::new(layout));

			if row.height == height {
				continue;
			}

			row.height = height;
			let extent = row.lead + height;
			self.heights.set(index, extent);
			let top = to_px(self.row_top(index));
			moved = Some(moved.map_or(top, |y: f32| y.min(top)));
		}

		Ok(moved)
	}

	/// Lay out row `index` if it has not been measured.
	pub(crate) fn ensure_row(
		&mut self,
		doc: &Document,
		measurer: &mut dyn crate::TextMeasurer,
		index: usize,
	) -> Result<(), LayoutError> {
		if self.flat.rows.get(index).is_some_and(Row::needs_layout) {
			self.lay_out_rows(doc, measurer, &[index])?;
		}

		Ok(())
	}

	/// Queue the marker text of list items whose first row is in `rows`.
	fn request_markers(&mut self, rows: &[usize]) {
		let mut requests = Vec::new();

		for &index in rows {
			self.collect_markers(index, &mut requests);
		}

		for (style, marker) in requests {
			if let Some(text) = self.marker_text(&marker) {
				let font = self.cache.font(&self.styles.get(style).text.font);
				self.cache.request(font, &text);
			}
		}
	}

	/// Push the markers drawn beside row `index` with the style that draws
	/// them: the row's own, then those of the enclosing list items it is the
	/// first row of, innermost first.
	fn collect_markers(&self, index: usize, requests: &mut Vec<(StyleId, Marker)>) {
		let row = &self.flat.rows[index];

		if let Some(marker) = &row.marker {
			requests.push((row.style, marker.clone()));
		}

		let mut span = row.span;

		while let Some(current) = span {
			let candidate = &self.flat.spans[current as usize];

			if candidate.first_row as usize != index {
				break;
			}

			if let Some(marker) = &candidate.marker {
				requests.push((candidate.style, marker.clone()));
			}

			span = candidate.parent;
		}
	}

	/// The text drawn for `marker`, or `None` for checkboxes.
	pub(crate) fn marker_text(&self, marker: &Marker) -> Option<Arc<str>> {
		match marker.kind {
			MarkerKind::Bullet => {
				let bullets = &self.theme().markers().bullets;
				bullets.get(marker.depth % bullets.len().max(1)).cloned()
			}
			MarkerKind::Ordered => Some(Arc::from(format!("{}.", marker.ordinal))),
			MarkerKind::Task => None,
		}
	}

	pub(crate) fn styles(&self) -> &Styles {
		&self.styles
	}

	#[expect(
		dead_code,
		reason = "the display list, the next layer of this stack, is the caller"
	)]
	pub(crate) fn cache(&self) -> &MeasureCache {
		&self.cache
	}

	pub(crate) fn rows(&self) -> &[Row] {
		&self.flat.rows
	}

	pub(crate) fn spans(&self) -> &[Span] {
		&self.flat.spans
	}

	pub(crate) fn row_index(&self, id: BlockId) -> Option<usize> {
		let key = self.flat.row_of.get(&id)?;
		self.flat
			.row_slots
			.get(*key as usize)
			.map(|index| *index as usize)
	}

	pub(crate) fn span_index(&self, id: BlockId) -> Option<usize> {
		let key = self.flat.span_of.get(&id)?;
		self.flat
			.span_slots
			.get(*key as usize)
			.map(|index| *index as usize)
	}

	pub(crate) fn style_of(&self, row: &Row) -> &BlockStyle {
		self.styles.get(row.style)
	}

	/// The space after the last row, closing the root's containers.
	fn tail(&self) -> i64 {
		self.flat.tail
	}

	/// The top of row `index`'s border box, in units.
	pub(crate) fn row_top(&self, index: usize) -> i64 {
		self.heights.prefix(index) + self.flat.rows.get(index).map_or(0, |row| row.lead)
	}

	pub(crate) fn row_rect(&self, index: usize) -> Rect {
		let Some(row) = self.flat.rows.get(index) else {
			return Rect::default();
		};

		Rect::new(
			row.x,
			to_px(self.row_top(index)),
			row.width,
			to_px(row.height),
		)
	}

	pub(crate) fn span_rect(&self, span: &Span) -> Rect {
		let top = self.row_top(span.first_row as usize) - span.top;
		let last = span.last_row as usize;
		let bottom =
			self.row_top(last) + self.heights.get(last) - self.flat.rows[last].lead + span.bottom;

		Rect::new(span.x, to_px(top), span.width, to_px(bottom - top))
	}

	/// The row containing `y`, if there are rows.
	pub(crate) fn row_at(&self, y: f32) -> Option<usize> {
		(!self.flat.rows.is_empty()).then(|| self.heights.find(to_units(y)))
	}

	/// Rows whose extent (leading space plus box) intersects `top..bottom`.
	pub(crate) fn rows_between(&self, top: f32, bottom: f32) -> impl Iterator<Item = usize> + '_ {
		let first = self.row_at(top.max(0.0)).unwrap_or(0);
		let bottom = to_units(bottom);
		let count = self.flat.rows.len();

		(first..count)
			.take_while(move |&index| self.heights.prefix(index) < bottom || index == first)
	}
}

/// The content-box width of a row whose border box is `width` wide.
pub(crate) fn content_width(style: &BlockStyle, width: f32) -> f32 {
	(width - style.inset_left() - style.inset_right()).max(0.0)
}
