//! Geometry queries within one text block: caret positions, hit testing,
//! and selection spans. Coordinates are relative to the block's content box.

use super::Line;
use super::Piece;
use super::TextLayout;
use super::items::ItemKind;
use super::items::grapheme_boundaries;
use super::items::slice;
use crate::Affinity;

/// Spans whose edges are closer than this many pixels touch, so float
/// rounding between adjacent pieces never splits a selection.
const TOUCHING_SPAN_GAP: f32 = 0.01;

impl TextLayout {
	/// The line holding the caret at `offset`. At a soft wrap the offset ends
	/// one line and starts the next; `affinity` picks which.
	pub(crate) fn line_index(&self, offset: u32, affinity: Affinity) -> usize {
		let index = self
			.lines
			.partition_point(|line| line.range.start <= offset)
			.saturating_sub(1);

		if affinity == Affinity::Upstream
			&& index > 0
			&& self.lines[index].range.start == offset
			&& self.lines[index - 1].soft
		{
			return index - 1;
		}

		index
	}

	/// The line whose box contains `y`, clamped to the first and last lines.
	pub(crate) fn line_at(&self, y: f32) -> usize {
		self.lines
			.partition_point(|line| line.top + line.height <= y)
			.min(self.lines.len().saturating_sub(1))
	}

	/// The x position of the caret at `offset` on line `line`.
	pub(crate) fn caret_x(&self, line: usize, offset: u32, affinity: Affinity) -> f32 {
		let Some(line) = self.lines.get(line) else {
			return 0.0;
		};
		let after = |piece: &&Piece| piece.range.start <= offset && offset < piece.range.end;
		let before = |piece: &&Piece| piece.range.start < offset && offset <= piece.range.end;

		let piece = match affinity {
			Affinity::Downstream => {
				line.pieces
					.iter()
					.find(after)
					.or_else(|| line.pieces.iter().find(before))
			}
			Affinity::Upstream => {
				line.pieces
					.iter()
					.find(before)
					.or_else(|| line.pieces.iter().find(after))
			}
		};

		piece.map_or_else(|| self.start_edge(), |piece| self.piece_x(piece, offset))
	}

	/// The x position of the start edge of a line, where the caret sits on
	/// an empty line.
	fn start_edge(&self) -> f32 {
		if self.rtl { self.width } else { 0.0 }
	}

	/// The caret position nearest to `x` on line `line`.
	pub(crate) fn hit(&self, line: usize, x: f32) -> (u32, Affinity) {
		let Some(line) = self.lines.get(line) else {
			return (0, Affinity::Downstream);
		};
		let pieces: Vec<&Piece> = line
			.pieces
			.iter()
			.filter(|piece| self.items[piece.item as usize].kind != ItemKind::Break)
			.collect();
		let Some(piece) = pieces
			.iter()
			.find(|piece| x < piece.x + piece.width)
			.or_else(|| pieces.last())
		else {
			return (line.range.start, Affinity::Downstream);
		};
		let mut best = (piece.range.start, f32::INFINITY);

		for boundary in self.boundaries(line, piece) {
			let distance = (self.piece_x(piece, boundary) - x).abs();

			if distance < best.1 {
				best = (boundary, distance);
			}
		}

		let offset = best.0;
		let affinity = if offset == piece.range.end && offset != piece.range.start {
			Affinity::Upstream
		} else {
			Affinity::Downstream
		};

		(offset, affinity)
	}

	/// Horizontal spans covering offsets `from..to` on line `line`, merged
	/// where they touch.
	pub(crate) fn spans(&self, line: usize, from: u32, to: u32) -> Vec<(f32, f32)> {
		let Some(line) = self.lines.get(line) else {
			return Vec::new();
		};
		let mut spans: Vec<(f32, f32)> = Vec::new();

		for piece in &line.pieces {
			let start = from.max(piece.range.start);
			let end = to.min(piece.range.end);

			if start >= end {
				continue;
			}

			let start_x = self.piece_x(piece, start);
			let end_x = self.piece_x(piece, end);

			push_merged(&mut spans, (start_x.min(end_x), start_x.max(end_x)));
		}

		spans
	}

	/// The x position of `offset` inside `piece`.
	fn piece_x(&self, piece: &Piece, offset: u32) -> f32 {
		let item = &self.items[piece.item as usize];
		let advance = item.span(&self.text, piece.range.start, offset);

		if piece.rtl {
			piece.x + piece.width - advance
		} else {
			piece.x + advance
		}
	}

	/// Caret stops inside `piece`: grapheme boundaries for text, both ends
	/// for atomic items.
	fn boundaries(&self, line: &Line, piece: &Piece) -> Vec<u32> {
		if !matches!(self.items[piece.item as usize].kind, ItemKind::Text { .. }) {
			return vec![piece.range.start, piece.range.end];
		}

		grapheme_boundaries(slice(&self.text, &line.range))
			.into_iter()
			.map(|offset| line.range.start + offset)
			.filter(|offset| piece.range.contains(offset) || *offset == piece.range.end)
			.collect()
	}
}

/// Append `span` to `spans`, extending the last span instead when they touch.
fn push_merged(spans: &mut Vec<(f32, f32)>, span: (f32, f32)) {
	match spans.last_mut() {
		Some(last) if (last.1 - span.0).abs() < TOUCHING_SPAN_GAP => last.1 = span.1,
		_ => spans.push(span),
	}
}
