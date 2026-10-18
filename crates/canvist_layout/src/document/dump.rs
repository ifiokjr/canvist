//! A plain-text dump of the layout tree for snapshot tests and debugging.

use std::fmt::Write;

use canvist_model::Schema;

use super::DocumentLayout;
use super::Span;
use crate::geometry::to_px;
use crate::text::ItemKind;
use crate::text::Line;
use crate::text::Piece;
use crate::text::TextLayout;
use crate::text::slice;

impl DocumentLayout {
	/// Describe every row, line, and piece, one per line of text:
	///
	/// ```text
	/// row 1.2 paragraph y=0 h=24 x=0 w=600
	///   line 0..12 y=0 h=24 base=18 w=115.2
	///     ltr x=0 w=57.6 "Hello "
	/// ```
	///
	/// Unmeasured rows show `estimated`. Containers are listed after the rows
	/// with the rows they span.
	#[must_use]
	pub fn dump(&self) -> String {
		let mut out = String::new();
		let schema = self.styles().schema();

		for index in 0..self.rows().len() {
			self.dump_row(&mut out, schema, index);
		}

		for span in self.spans() {
			self.dump_span(&mut out, schema, span);
		}

		let _ = writeln!(
			out,
			"height {}",
			to_px(self.heights.total() + self.flat.tail)
		);

		out
	}

	/// Write row `index` and, once measured, its lines and pieces.
	fn dump_row(&self, out: &mut String, schema: &Schema, index: usize) {
		let row = &self.rows()[index];
		let rect = self.row_rect(index);
		let name = &schema.block(row.block_type).name;
		let _ = writeln!(
			out,
			"row {} {name} y={} h={} x={} w={}{}",
			row.block,
			round(rect.y),
			round(rect.height),
			round(rect.x),
			round(rect.width),
			if row.needs_layout() { " estimated" } else { "" }
		);

		let Some(layout) = &row.layout else {
			return;
		};

		for line in &layout.lines {
			dump_line(out, layout, line);
		}
	}

	/// Write a container with the rows it spans and its list marker, if any.
	fn dump_span(&self, out: &mut String, schema: &Schema, span: &Span) {
		let rect = self.span_rect(span);
		let name = &schema.block(span.block_type).name;
		let marker = span
			.marker
			.as_ref()
			.map(|marker| format!(" marker={:?}#{}", marker.kind, marker.ordinal))
			.unwrap_or_default();
		let _ = writeln!(
			out,
			"span {} {name} rows={}..={} y={} h={} x={} w={}{marker}",
			span.block,
			span.first_row,
			span.last_row,
			round(rect.y),
			round(rect.height),
			round(rect.x),
			round(rect.width)
		);
	}
}

/// Write a line of a text layout followed by its pieces.
fn dump_line(out: &mut String, layout: &TextLayout, line: &Line) {
	let _ = writeln!(
		out,
		"  line {}..{}{} y={} h={} base={} w={}",
		line.range.start,
		line.range.end,
		if line.soft { " soft" } else { "" },
		line.top,
		line.height,
		line.baseline,
		round(line.width)
	);

	for piece in &line.pieces {
		dump_piece(out, layout, piece);
	}
}

/// Write one piece with its direction, item kind, extent, and text.
fn dump_piece(out: &mut String, layout: &TextLayout, piece: &Piece) {
	let item = &layout.items[piece.item as usize];

	let kind = match item.kind {
		ItemKind::Text { .. } => "",
		ItemKind::Tab => " tab",
		ItemKind::Break => " break",
		ItemKind::Embed(_) => " embed",
	};

	let _ = writeln!(
		out,
		"    {}{kind} x={} w={} {:?}",
		if piece.rtl { "rtl" } else { "ltr" },
		round(piece.x),
		round(piece.width),
		slice(&layout.text, &piece.range)
	);
}

/// Round to 1/100 px so snapshots ignore float noise.
fn round(value: f32) -> f32 {
	(value * 100.0).round() / 100.0 + 0.0
}
