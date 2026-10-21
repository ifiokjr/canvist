//! Table grids: cells side by side, rows as tall as their tallest cell,
//! merged cells, incremental relayout, hit testing, vertical motion, grid
//! lines, and HTML that a browser lays out the same way.

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use std::sync::Arc;

use canvist_layout::Caret;
use canvist_layout::DisplayItem;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::HtmlOptions;
use canvist_layout::LayoutOptions;
use canvist_layout::RectKind;
use canvist_layout::Theme;
use canvist_layout::VerticalDirection;
use canvist_layout::to_html;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::MarkSet;
use canvist_model::Point;
use serde_json::json;
use support::doc;
use support::layout;
use support::paragraph;
use support::table;

const WIDTH: f32 = 320.0;
/// The height of a body line: 16 px at a line height of 1.5.
const LINE: f32 = 24.0;
/// A cell's vertical padding: 0.375 rem.
const CELL_PADDING: f32 = 6.0;
/// The grid line width.
const RULE: f32 = 1.0;

/// The text block of cell `column` in table row `row` of the first table.
fn cell_text(doc: &Document, row: usize, column: usize) -> BlockId {
	let table = doc
		.children(None)
		.iter()
		.copied()
		.find(|&id| {
			doc.block(id)
				.is_some_and(|block| &*doc.schema().block(block.kind()).name == "table")
		})
		.expect("a table");
	let row = doc.children(Some(table))[row];
	let cell = doc.children(Some(row))[column];
	doc.children(Some(cell))[0]
}

fn rect(layout: &DocumentLayout, block: BlockId) -> canvist_layout::Rect {
	layout.block_rect(block).expect("a laid-out block")
}

#[test]
fn cells_sit_side_by_side_in_even_columns() {
	let doc = doc(json!([table(&[&["Left", "Right"], &["Below", "Too"]])]));
	let layout = layout(&doc, WIDTH);
	let left = rect(&layout, cell_text(&doc, 0, 0));
	let right = rect(&layout, cell_text(&doc, 0, 1));
	let below = rect(&layout, cell_text(&doc, 1, 0));
	let column = (WIDTH - RULE) / 2.0;

	assert_eq!(left.y, right.y, "a row's cells share a top");
	assert_eq!(right.x - left.x, column, "columns split the width evenly");
	assert_eq!(left.y, RULE + CELL_PADDING);
	assert_eq!(
		below.y - left.y,
		LINE + 2.0 * CELL_PADDING + RULE,
		"the next row starts a cell's height and a line further down"
	);
	assert_eq!(
		layout.height(),
		2.0 * (LINE + 2.0 * CELL_PADDING + RULE) + RULE
	);
}

#[test]
fn a_row_is_as_tall_as_its_tallest_cell() {
	let long = "a long cell that wraps onto several lines in its column";
	let doc = doc(json!([table(&[&["Short", long], &["Next", "Row"]])]));
	let layout = layout(&doc, WIDTH);
	let lines = layout.line_count(cell_text(&doc, 0, 1)).expect("measured") as f32;
	let short = rect(&layout, cell_text(&doc, 0, 0));
	let next = rect(&layout, cell_text(&doc, 1, 0));

	assert!(lines > 1.0);
	assert_eq!(next.y - short.y, lines * LINE + 2.0 * CELL_PADDING + RULE);
}

#[test]
fn merged_cells_span_columns_and_rows() {
	// | wide wide | c |
	// | d | e     | c |
	let doc = doc(json!([table(&[&["wide|2x1", "c|1x2"], &["d", "e"]])]));
	let layout = layout(&doc, WIDTH);
	let wide = rect(&layout, cell_text(&doc, 0, 0));
	let tall = rect(&layout, cell_text(&doc, 0, 1));
	let d = rect(&layout, cell_text(&doc, 1, 0));
	let e = rect(&layout, cell_text(&doc, 1, 1));
	let column = (WIDTH - RULE) / 3.0;

	assert_eq!(
		wide.width,
		2.0 * column - RULE - 16.0,
		"two columns, less padding"
	);
	assert!((tall.x - (2.0 * column + RULE + 8.0)).abs() < 0.01);
	assert_eq!(d.y, e.y);
	assert!((e.x - d.x - column).abs() < 0.01);
	insta::assert_snapshot!(layout.dump());
}

#[test]
fn colwidth_fixes_a_column_and_the_rest_share_what_is_left() {
	let mut content = table(&[&["Fixed", "Free", "Free"]]);
	content["content"][0]["content"][0]["attrs"]["colwidth"] = json!([100]);
	let doc = doc(json!([content]));
	let layout = layout(&doc, WIDTH);
	let first = rect(&layout, cell_text(&doc, 0, 0));
	let second = rect(&layout, cell_text(&doc, 0, 1));
	let third = rect(&layout, cell_text(&doc, 0, 2));

	assert_eq!(second.x - first.x, 100.0);
	assert_eq!(third.x - second.x, (WIDTH - RULE - 100.0) / 2.0);
}

#[test]
fn rows_after_a_table_follow_its_grid() {
	let doc = doc(json!([
		paragraph("Before"),
		table(&[&["a", "b"]]),
		paragraph("After")
	]));
	let layout = layout(&doc, WIDTH);
	let after = doc.children(None)[2];
	let table = doc.children(None)[1];
	let grid = rect(&layout, table);

	assert_eq!(grid.height, LINE + 2.0 * CELL_PADDING + 2.0 * RULE);
	assert!(rect(&layout, after).y >= grid.y + grid.height);
	assert_eq!(layout.height(), rect(&layout, after).y + LINE);
}

/// Lay `doc` out, apply `edit`, and require the incremental relayout to
/// match a fresh one.
fn relayout_matches_fresh(
	mut doc: Document,
	edit: impl FnOnce(&mut Document) -> canvist_model::Commit,
) {
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(WIDTH),
	);
	layout.layout_all(&doc, &mut measurer).expect("measured");
	let commit = edit(&mut doc);
	layout.apply(&doc, &commit);
	layout.layout_all(&doc, &mut measurer).expect("measured");

	assert_eq!(layout.dump(), support::layout(&doc, WIDTH).dump());
}

#[test]
fn typing_in_a_cell_relays_out_its_row_like_a_fresh_layout() {
	let doc = doc(json!([
		table(&[&["a", "b"], &["c", "d"]]),
		paragraph("After")
	]));
	let block = cell_text(&doc, 0, 1);

	relayout_matches_fresh(doc, |doc| {
		let mut transaction = doc.transaction();
		transaction
			.insert_text(
				Point::new(block, 1),
				" grows until it wraps onto a second line",
				MarkSet::empty(),
			)
			.expect("valid text");
		transaction.commit()
	});
}

#[test]
fn splitting_a_cell_paragraph_relays_out_like_a_fresh_layout() {
	let doc = doc(json!([
		paragraph("Before"),
		table(&[&["a", "bc"], &["d", "e"]]),
		paragraph("After")
	]));
	let block = cell_text(&doc, 0, 1);
	let paragraph = doc.schema().default_text_block();

	relayout_matches_fresh(doc, |doc| {
		let mut transaction = doc.transaction();
		transaction
			.split(Point::new(block, 1), paragraph, canvist_model::Attrs::new())
			.expect("valid split");
		transaction.commit()
	});
}

#[test]
fn a_click_lands_in_the_cell_under_it() {
	let doc = doc(json!([table(&[&["Left", "Right"]])]));
	let layout = layout(&doc, WIDTH);
	let right = rect(&layout, cell_text(&doc, 0, 1));
	let caret = layout
		.hit_test(right.x + 1.0, right.y + 1.0)
		.expect("a caret");

	assert_eq!(caret.point.block, cell_text(&doc, 0, 1));
	assert_eq!(
		layout.hit_block(right.x + 1.0, right.y + 1.0),
		Some(cell_text(&doc, 0, 1))
	);
}

#[test]
fn arrow_down_moves_to_the_cell_below_then_out_of_the_table() {
	let doc = doc(json!([
		table(&[&["a", "b"], &["c", "d"]]),
		paragraph("After")
	]));
	let mut layout = layout(&doc, WIDTH);
	let mut measurer = FixedMeasurer::new();
	let start = Caret::new(Point::new(cell_text(&doc, 0, 1), 0));
	let mut navigator = layout.navigator(&doc, &mut measurer);

	let below = navigator
		.vertical(start, VerticalDirection::Down, None)
		.expect("laid out")
		.expect("a line below");
	assert_eq!(below.caret.point.block, cell_text(&doc, 1, 1));

	let out = navigator
		.vertical(below.caret, VerticalDirection::Down, Some(below.goal_x))
		.expect("laid out")
		.expect("a line below");
	assert_eq!(out.caret.point.block, doc.children(None)[1]);

	let back = navigator
		.vertical(below.caret, VerticalDirection::Up, Some(below.goal_x))
		.expect("laid out")
		.expect("a line above");
	assert_eq!(back.caret.point.block, cell_text(&doc, 0, 1));
}

#[test]
fn the_grid_draws_lines_around_every_cell() {
	let doc = doc(json!([table(&[&["a", "b"], &["c", "d"]])]));
	let layout = layout(&doc, WIDTH);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));
	let lines: Vec<_> = list
		.items
		.iter()
		.filter(|item| {
			matches!(
				item,
				DisplayItem::Rect {
					kind: RectKind::Border,
					..
				}
			)
		})
		.collect();

	assert_eq!(lines.len(), 16, "four edges for each of four cells");
}

#[test]
fn html_writes_a_fixed_table_with_layouts_column_widths() {
	let mut content = table(&[&["wide|2x1"], &["a", "b"]]);
	content["content"][1]["content"][0]["type"] = json!("tableHeader");
	let doc = doc(json!([content]));
	let theme = Arc::new(Theme::starter_kit());
	let html = to_html(&doc, &theme, &HtmlOptions::new(WIDTH));

	assert!(
		html.html.contains(
			"<colgroup><col style=\"width:159.5px\"><col style=\"width:159.5px\"></colgroup>"
		),
		"{}",
		html.html
	);
	assert!(html.html.contains("colspan=\"2\""), "{}", html.html);
	assert!(html.html.contains("<th "), "{}", html.html);
	assert!(html.html.contains("width:320px"), "{}", html.html);
	assert!(
		html.css.contains("border-collapse:collapse"),
		"{}",
		html.css
	);
	assert!(
		html.css.contains(">tbody>tr>.cv-b{border:1px solid"),
		"{}",
		html.css
	);
}

#[test]
fn html_header_cells_say_which_cells_they_label() {
	let mut content = table(&[
		&["", "Mon", "Tue"],
		&["Ada", "1", "2"],
		&["Bob", "3", "4|1x1"],
	]);

	for column in 0..3 {
		content["content"][0]["content"][column]["type"] = json!("tableHeader");
	}

	for row in 1..3 {
		content["content"][row]["content"][0]["type"] = json!("tableHeader");
	}

	// A header in the middle of the table labels nothing in particular.
	content["content"][2]["content"][2]["type"] = json!("tableHeader");
	let doc = doc(json!([content]));
	let html = to_html(
		&doc,
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(WIDTH),
	)
	.html;
	let scopes: Vec<&str> = html
		.split("<th ")
		.skip(1)
		.map(|tag| {
			tag.split('>')
				.next()
				.and_then(|tag| tag.split("scope=\"").nth(1))
				.and_then(|scope| scope.split('"').next())
				.unwrap_or("none")
		})
		.collect();

	assert_eq!(
		scopes,
		["col", "col", "col", "row", "row", "none"],
		"{html}"
	);
	assert!(!html.contains("<td scope"), "{html}");
	assert!(
		html.contains("<table ") && html.contains("<tbody>"),
		"{html}"
	);
}
