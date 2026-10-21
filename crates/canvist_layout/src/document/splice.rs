//! Structural relayout without re-flattening the whole document.
//!
//! A structural commit (Enter, Backspace across blocks, inserting, removing,
//! or moving blocks) changes a few root-level subtrees. Only those are
//! flattened again, and their rows and spans are spliced into place. The
//! rows and spans after them keep their layouts, styles, and hash entries;
//! they only have the indices they hold shifted. That costs `O(root
//! siblings)` to find the changed run, `O(changed subtrees)` to flatten it,
//! and one pass of integer updates over the rest, with no hashing, style
//! resolution, or measurement.

use canvist_model::BlockId;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::Step;

use super::DocumentLayout;
use super::rows;
use super::rows::Flat;
use super::rows::GridPart;
use super::rows::Region;
use super::rows::RootExtent;
use super::rows::Row;
use super::rows::Seed;
use super::rows::Span;
use crate::geometry::to_units;

/// The run of root-level blocks a commit changed: `start..new_end` in the
/// new document, replacing `start..old_end` in the old one.
#[derive(Clone, Copy, Debug)]
struct Window {
	start: usize,
	new_end: usize,
	old_end: usize,
}

/// Where a window's rows and spans start in the flattened document.
#[derive(Clone, Copy, Debug)]
struct Origin {
	row: usize,
	span: usize,
}

impl DocumentLayout {
	/// Re-flatten the root-level blocks `commit` changed, plus the roots
	/// holding `anchors` (blocks whose style changed). Returns `false` when
	/// the change can't be localized, so the caller must rebuild everything.
	pub(super) fn splice(&mut self, doc: &Document, commit: &Commit, anchors: &[BlockId]) -> bool {
		let Some(window) = self.window(doc, commit, anchors) else {
			return false;
		};
		let (origin, old_rows, old_spans) = self.old_range(window);
		let seed = self.seed(window.start);
		let old_previous = window
			.old_end
			.checked_sub(1)
			.map(|index| self.flat.roots[index]);
		let previous = self.take_range(origin, old_rows, old_spans);
		let region = Region {
			roots: &doc.children(None)[window.start..window.new_end],
			width: self.options.width,
			seed,
		};
		let chunk = rows::flatten_roots(doc, &mut self.styles, &region, previous);

		self.insert_chunk(window, origin, (old_rows, old_spans), chunk, old_previous);
		self.rebuild_heights();
		true
	}

	/// The changed run of root blocks: what differs between the old and new
	/// root lists, widened to cover every root holding an anchor.
	fn window(&self, doc: &Document, commit: &Commit, anchors: &[BlockId]) -> Option<Window> {
		let new_roots = doc.children(None);
		let old_roots = &self.flat.roots;
		let prefix = new_roots
			.iter()
			.zip(old_roots)
			.take_while(|(new, old)| **new == old.block)
			.count();
		let longest = new_roots.len().min(old_roots.len()) - prefix;
		let suffix = new_roots
			.iter()
			.rev()
			.zip(old_roots.iter().rev())
			.take(longest)
			.take_while(|(new, old)| **new == old.block)
			.count();
		let mut start = prefix;
		let mut end = new_roots.len() - suffix;
		let step_anchors = commit.steps.iter().flat_map(step_anchors);

		for anchor in step_anchors.chain(anchors.iter().copied()) {
			let index = root_index(doc, anchor)?;
			start = start.min(index);
			end = end.max(index + 1);
		}

		let old_end = old_roots.len().checked_sub(new_roots.len() - end)?;

		Some(Window {
			start,
			new_end: end,
			old_end,
		})
	}

	/// Where the window starts, and how many rows and spans it held.
	fn old_range(&self, window: Window) -> (Origin, usize, usize) {
		let count = |extents: &[RootExtent]| {
			extents.iter().fold((0, 0), |(rows, spans), extent| {
				(rows + extent.rows as usize, spans + extent.spans as usize)
			})
		};
		let (row, span) = count(&self.flat.roots[..window.start]);
		let (rows, spans) = count(&self.flat.roots[window.start..window.old_end]);

		(Origin { row, span }, rows, spans)
	}

	/// The space the root before `start` leaves owing to the first row of
	/// the window.
	fn seed(&self, start: usize) -> Seed {
		let Some(previous) = start.checked_sub(1).map(|index| self.flat.roots[index]) else {
			return Seed::default();
		};

		Seed {
			pending: previous.closing,
			previous_after: Some(self.styles.get(previous.style).space_after),
		}
	}

	/// Forget the blocks of the window's old rows and spans, and copy the
	/// rows so their layouts can be reused. The rows stay in place until
	/// [`Self::insert_chunk`] replaces them, so the tail moves only once.
	fn take_range(&mut self, origin: Origin, rows: usize, spans: usize) -> Vec<Row> {
		let previous = self.flat.rows[origin.row..origin.row + rows].to_vec();

		for row in &previous {
			self.flat.row_of.remove(&row.block);
		}

		for span in &self.flat.spans[origin.span..origin.span + spans] {
			self.flat.span_of.remove(&span.block);
		}

		previous
	}

	/// Splice a freshly flattened chunk in where the window's old rows and
	/// spans were, and shift everything after it.
	fn insert_chunk(
		&mut self,
		window: Window,
		origin: Origin,
		old_counts: (usize, usize),
		chunk: Flat,
		old_previous: Option<RootExtent>,
	) {
		let delta = (
			chunk.rows.len() as i64 - old_counts.0 as i64,
			chunk.spans.len() as i64 - old_counts.1 as i64,
		);
		let next_row = origin.row + chunk.rows.len();
		let next_span = origin.span + chunk.spans.len();
		let reaches_end = window.new_end == window.start + chunk.roots.len()
			&& window.old_end == self.flat.roots.len();

		if reaches_end {
			self.flat.tail = chunk.tail;
		}

		let new_previous = chunk.roots.last().copied();
		self.place_rows(origin, old_counts.0, chunk.rows);
		self.place_spans(origin, old_counts.1, chunk.spans);
		self.shift_after(next_row, next_span, delta);
		self.reindex(origin);
		let chunk_roots = chunk.roots.len();
		self.flat
			.roots
			.splice(window.start..window.old_end, chunk.roots);

		let new_previous = new_previous.or_else(|| {
			window
				.start
				.checked_sub(1)
				.map(|index| self.flat.roots[index])
		});
		let next = self.flat.roots.get(window.start + chunk_roots).copied();
		self.fix_next_lead(next_row, next, (old_previous, new_previous));
	}

	/// Replace the `replaced` old rows at the origin with the chunk's,
	/// translating their local span indices and keys into the document's.
	fn place_rows(&mut self, origin: Origin, replaced: usize, rows: Vec<Row>) {
		let key_base = self.flat.row_slots.len() as u32;
		let count = rows.len();
		let translated = rows.into_iter().enumerate().map(|(index, mut row)| {
			row.key = key_base + index as u32;
			row.span = row.span.map(|span| span + origin.span as u32);
			row.table = row.table.map(|table| table + origin.span as u32);
			row
		});

		self.flat
			.rows
			.splice(origin.row..origin.row + replaced, translated);
		self.flat.row_slots.resize(key_base as usize + count, 0);

		for row in &self.flat.rows[origin.row..origin.row + count] {
			self.flat.row_of.insert(row.block, row.key);
		}
	}

	/// Replace the `replaced` old spans at the origin with the chunk's,
	/// translating their row and parent indices and keys into the document's.
	fn place_spans(&mut self, origin: Origin, replaced: usize, spans: Vec<Span>) {
		let key_base = self.flat.span_slots.len() as u32;
		let count = spans.len();
		let translated = spans.into_iter().enumerate().map(|(index, mut span)| {
			span.key = key_base + index as u32;
			span.first_row += origin.row as u32;
			span.last_row += origin.row as u32;
			span.parent = span.parent.map(|parent| parent + origin.span as u32);

			if let Some(table) = span.grid.as_mut().and_then(GridPart::table_mut) {
				*table += origin.span as u32;
			}

			span
		});

		self.flat
			.spans
			.splice(origin.span..origin.span + replaced, translated);
		self.flat.span_slots.resize(key_base as usize + count, 0);

		for span in &self.flat.spans[origin.span..origin.span + count] {
			self.flat.span_of.insert(span.block, span.key);
		}
	}

	/// Shift the indices held by the rows and spans after the chunk by
	/// `delta` (rows, spans).
	fn shift_after(&mut self, rows_from: usize, spans_from: usize, delta: (i64, i64)) {
		let shift = |value: u32, by: i64| (i64::from(value) + by) as u32;
		let flat = &mut self.flat;

		for row in &mut flat.rows[rows_from..] {
			row.span = row.span.map(|span| shift(span, delta.1));
			row.table = row.table.map(|table| shift(table, delta.1));
		}

		for span in &mut flat.spans[spans_from..] {
			span.first_row = shift(span.first_row, delta.0);
			span.last_row = shift(span.last_row, delta.0);
			span.parent = span.parent.map(|parent| shift(parent, delta.1));

			if let Some(table) = span.grid.as_mut().and_then(GridPart::table_mut) {
				*table = shift(*table, delta.1);
			}
		}
	}

	/// Point the key of every row and span from `origin` onwards at its
	/// current index.
	fn reindex(&mut self, origin: Origin) {
		let flat = &mut self.flat;

		for (index, row) in flat.rows.iter().enumerate().skip(origin.row) {
			flat.row_slots[row.key as usize] = index as u32;
		}

		for (index, span) in flat.spans.iter().enumerate().skip(origin.span) {
			flat.span_slots[span.key as usize] = index as u32;
		}
	}

	/// Correct the leading space of the first row after the window, which
	/// includes the closing padding and spacing owed by the root before it.
	fn fix_next_lead(
		&mut self,
		next_row: usize,
		next: Option<RootExtent>,
		previous: (Option<RootExtent>, Option<RootExtent>),
	) {
		let Some(next) = next else {
			return;
		};
		let owed = |root: Option<RootExtent>| {
			root.map_or(0, |root| {
				let after = self.styles.get(root.style).space_after;
				let before = self.styles.get(next.style).space_before;
				root.closing + to_units(after.max(before))
			})
		};
		let change = owed(previous.1) - owed(previous.0);
		let table = self.flat.rows.get(next_row).and_then(Row::table);

		// A table's first row has its leading space in the table's grid.
		if let Some(span) = table.and_then(|table| self.flat.spans.get_mut(table as usize))
			&& span.first_row as usize == next_row
			&& let Some(GridPart::Table(grid)) = &mut span.grid
		{
			grid.lead += change;
		} else if let Some(row) = self.flat.rows.get_mut(next_row) {
			row.lead += change;
		}
	}
}

/// Blocks a step changed the children or style of, whose root subtrees must
/// be flattened again.
fn step_anchors(step: &Step) -> Vec<BlockId> {
	match step {
		Step::Split { block, .. } | Step::Join { block, .. } | Step::SetBlock { block, .. } => {
			vec![*block]
		}
		Step::InsertBlocks { parent, blocks, .. } => {
			parent.map_or_else(
				|| blocks.iter().map(|tree| tree.id).collect(),
				|parent| vec![parent],
			)
		}
		Step::RemoveBlocks { parent, .. } => parent.iter().copied().collect(),
		Step::MoveBlocks {
			parent, to_parent, ..
		} => parent.iter().chain(to_parent).copied().collect(),
		_ => Vec::new(),
	}
}

/// The index among the root's children of the root-level ancestor of `id`,
/// or `None` if `id` is no longer in the document.
fn root_index(doc: &Document, id: BlockId) -> Option<usize> {
	let mut current = id;

	while let Some(parent) = doc.block(current)?.parent() {
		current = parent;
	}

	doc.index_in_parent(current)
}
