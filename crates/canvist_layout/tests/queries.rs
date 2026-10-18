//! Geometry queries, navigation, virtualization, and incremental relayout.
//!
//! Expected geometry is written in terms of the [`FixedMeasurer`] metrics
//! below, so each number says which text or line it comes from.

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use std::sync::Arc;

use canvist_layout::Affinity;
use canvist_layout::Caret;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::LayoutError;
use canvist_layout::LayoutOptions;
use canvist_layout::LineEdge;
use canvist_layout::MeasureBatch;
use canvist_layout::MeasureOutput;
use canvist_layout::TextMeasurer;
use canvist_layout::Theme;
use canvist_layout::VerticalDirection;
use canvist_model::Attrs;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Selection;
use serde_json::json;
use support::doc;
use support::layout;
use support::paragraphs;

/// The advance of most characters in the 16 px body font: 0.6 em.
const ADVANCE: f32 = 9.6;
/// The advance of an emoji in the body font: 1 em.
const EMOJI_ADVANCE: f32 = 16.0;
/// The height of a body line: 16 px at a line height of 1.5.
const LINE: f32 = 24.0;
/// The space between consecutive paragraphs.
const PARAGRAPH_GAP: f32 = 12.0;
/// Wide enough that no paragraph in these tests wraps.
const WIDE: f32 = 600.0;
/// Fits "aaa bbb " (its trailing space hangs past the edge) but not
/// "aaa bbb ccc", so that text wraps after the second word.
const NARROW: f32 = 70.0;

fn caret(block: canvist_model::BlockId, offset: u32, affinity: Affinity) -> Caret {
	Caret::with_affinity(Point::new(block, offset), affinity)
}

/// The centre of a caret rectangle.
fn centre(rect: canvist_layout::Rect) -> (f32, f32) {
	(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

/// The byte offset just past `text`.
fn offset(text: &str) -> u32 {
	text.len() as u32
}

/// The width of `text` in the body font, counting every character as one
/// [`ADVANCE`].
///
/// Rounded like [`support::r`], so it is the same `f32` as the decimal
/// literal for that width (`76.8` for eight characters) rather than a value
/// one ulp away from it.
fn width(text: &str) -> f32 {
	support::r(text.chars().count() as f32 * ADVANCE)
}

/// The vertical middle of line `line` of the first paragraph.
fn line_middle(line: u16) -> f32 {
	LINE * f32::from(line) + LINE / 2.0
}

/// Points a quarter of the way into the span `start..start + extent` from
/// each edge, so each falls clearly on one side of the span's middle.
fn quarters(start: f32, extent: f32) -> (f32, f32) {
	let quarter = extent / 4.0;
	(start + quarter, start + extent - quarter)
}

/// The caret offset that a hit at `(x, y)` resolves to.
fn hit_offset(layout: &DocumentLayout, x: f32, y: f32) -> Option<u32> {
	layout.hit_test(x, y).map(|caret| caret.point.offset)
}

#[test]
fn affinity_picks_the_line_at_a_soft_wrap() {
	let first_line = "aaa bbb ";
	let doc = paragraphs(&["aaa bbb ccc"]);
	let layout = layout(&doc, NARROW);
	let block = doc.children(None)[0];
	let wrap = offset(first_line);
	let up = layout
		.caret_rect(caret(block, wrap, Affinity::Upstream))
		.expect("caret");
	let down = layout
		.caret_rect(caret(block, wrap, Affinity::Downstream))
		.expect("caret");
	let past_the_edge = 2.0 * NARROW;

	assert_eq!(layout.line_count(block), Some(2));
	assert_eq!(centre(up), (width(first_line), line_middle(0)));
	assert_eq!(centre(down), (0.0, line_middle(1)));
	assert_eq!(
		layout.hit_test(past_the_edge, line_middle(0)),
		Some(caret(block, wrap, Affinity::Upstream))
	);
	assert_eq!(
		layout.hit_test(0.0, line_middle(1)),
		Some(caret(block, wrap, Affinity::Downstream))
	);
}

#[test]
fn hit_testing_snaps_to_grapheme_boundaries() {
	let before_accent = "caf";
	let accented = "e\u{301}";
	let before_family = format!("{before_accent}{accented} ");
	let family = "👨\u{200D}👩\u{200D}👧";
	let doc = paragraphs(&[&format!("{before_family}{family}!")]);
	let layout = layout(&doc, WIDE);
	let y = line_middle(0);

	// The combining accent advances nothing, so "é" is one character wide
	// and never splits into two caret stops: a hit either side of its middle
	// lands before or after the whole cluster.
	let (left, right) = quarters(width(before_accent), ADVANCE);
	assert_eq!(hit_offset(&layout, left, y), Some(offset(before_accent)));
	assert_eq!(
		hit_offset(&layout, right, y),
		Some(offset(before_accent) + offset(accented))
	);

	// The family is one grapheme of three emoji, and its joiners advance
	// nothing; "cafe " has the same width as the text before it.
	let (left, right) = quarters(width("cafe "), 3.0 * EMOJI_ADVANCE);
	assert_eq!(hit_offset(&layout, left, y), Some(offset(&before_family)));
	assert_eq!(
		hit_offset(&layout, right, y),
		Some(offset(&before_family) + offset(family))
	);
}

#[test]
fn right_to_left_hits_mirror() {
	let text = "אבג";
	let first_two = "אב";
	let edge = 100.0;
	let doc = paragraphs(&[text]);
	let layout = layout(&doc, edge);
	let block = doc.children(None)[0];
	let y = line_middle(0);

	// The paragraph is right-aligned, so "א" is the rightmost letter, ending
	// at the edge, and "ב" sits immediately to its left.
	let (_, first_right) = quarters(edge - ADVANCE, ADVANCE);
	let (second_left, _) = quarters(edge - width(first_two), ADVANCE);
	assert_eq!(hit_offset(&layout, first_right, y), Some(0));
	assert_eq!(hit_offset(&layout, second_left, y), Some(offset(first_two)));
	assert_eq!(hit_offset(&layout, 0.0, y), Some(offset(text)));
	assert_eq!(
		layout
			.caret_rect(caret(block, 0, Affinity::Downstream))
			.map(centre),
		Some((edge, y))
	);
}

#[test]
fn selection_spans_blocks_and_extends_wrapped_lines() {
	let doc = paragraphs(&["aaa bbb ccc", "dd"]);
	let layout = layout(&doc, NARROW);
	let [first, second] = [doc.children(None)[0], doc.children(None)[1]];
	let unselected = "aaa ";
	let selected_in_second = "d";
	let selection = Selection::text(
		Point::new(first, offset(unselected)),
		Point::new(second, offset(selected_in_second)),
	);
	let rects: Vec<_> = layout
		.selection_rects(&selection)
		.into_iter()
		.map(|rect| {
			(
				support::r(rect.x),
				support::r(rect.y),
				support::r(rect.width),
			)
		})
		.collect();
	let second_paragraph = 2.0 * LINE + PARAGRAPH_GAP;

	// The selection runs past the end of the first paragraph, so its last
	// line gets a second rectangle reaching to the edge.
	assert_eq!(
		rects,
		[
			(width(unselected), 0.0, width("bbb ")),
			(0.0, LINE, width("ccc")),
			(width("ccc"), LINE, support::r(NARROW - width("ccc"))),
			(0.0, second_paragraph, width(selected_in_second))
		]
	);
	assert!(
		layout
			.selection_rects(&Selection::caret(Point::new(first, 2)))
			.is_empty()
	);
}

#[test]
fn navigation_keeps_the_goal_column() {
	let doc = paragraphs(&["aaaaaaaaaa", "b", "cccccccccc"]);
	let mut layout = layout(&doc, WIDE);
	let blocks = doc.children(None).to_vec();
	let mut measurer = FixedMeasurer::new();
	let mut navigator = layout.navigator(&doc, &mut measurer);
	// The goal column is past the end of the short middle paragraph.
	let before_goal = "aaaaaaaa";
	let start = caret(blocks[0], offset(before_goal), Affinity::Downstream);

	let down = navigator
		.vertical(start, VerticalDirection::Down, None)
		.expect("laid out")
		.expect("there is a line below");
	assert_eq!(down.caret.point, Point::new(blocks[1], offset("b")));
	assert_eq!(support::r(down.goal_x), width(before_goal));

	let again = navigator
		.vertical(down.caret, VerticalDirection::Down, Some(down.goal_x))
		.expect("laid out")
		.expect("there is a line below");
	assert_eq!(
		again.caret.point,
		Point::new(blocks[2], offset(before_goal))
	);

	assert_eq!(
		navigator.vertical(again.caret, VerticalDirection::Down, None),
		Ok(None)
	);
	assert_eq!(
		navigator.vertical(start, VerticalDirection::Up, None),
		Ok(None)
	);
}

#[test]
fn line_edges_follow_wraps() {
	let text = "aaa bbb ccc";
	let doc = paragraphs(&[text]);
	let mut layout = layout(&doc, NARROW);
	let block = doc.children(None)[0];
	let mut measurer = FixedMeasurer::new();
	let mut navigator = layout.navigator(&doc, &mut measurer);

	let wrap = offset("aaa bbb ");
	let in_first_line = caret(block, offset("aaa b"), Affinity::Downstream);
	let in_second_line = caret(block, wrap + 1, Affinity::Downstream);

	assert_eq!(
		navigator.line_edge(in_first_line, LineEdge::Start),
		Ok(caret(block, 0, Affinity::Downstream))
	);
	assert_eq!(
		navigator.line_edge(in_first_line, LineEdge::End),
		Ok(caret(block, wrap, Affinity::Upstream))
	);
	assert_eq!(
		navigator.line_edge(in_second_line, LineEdge::End),
		Ok(caret(block, offset(text), Affinity::Downstream))
	);
}

#[test]
fn vertical_motion_skips_leaf_blocks() {
	let above = "above";
	let doc = doc(json!([
		{ "type": "paragraph", "content": [{ "type": "text", "text": above }] },
		{ "type": "horizontalRule" },
		{ "type": "paragraph", "content": [{ "type": "text", "text": "below" }] }
	]));
	let mut layout = layout(&doc, WIDE);
	let blocks = doc.children(None).to_vec();
	let mut measurer = FixedMeasurer::new();
	let motion = layout
		.navigator(&doc, &mut measurer)
		.vertical(
			caret(blocks[0], 2, Affinity::Downstream),
			VerticalDirection::Down,
			None,
		)
		.expect("laid out")
		.expect("there is a line below");
	let rule_top = layout.block_rect(blocks[1]).expect("rule").y;

	assert_eq!(motion.caret.point, Point::new(blocks[2], 2));
	assert_eq!(
		layout.hit_test(10.0, rule_top),
		Some(caret(blocks[0], offset(above), Affinity::Downstream))
	);
}

#[test]
fn virtualization_measures_one_viewport_in_one_call() {
	const PARAGRAPH_COUNT: usize = 10_000;
	const VIEWPORT: f32 = 800.0;
	/// Far fewer than the document holds, but enough for one viewport.
	const MAX_LAID_OUT: usize = 30;
	/// Below the estimate for 10,000 one-line paragraphs, each a 24 px line
	/// plus a 12 px gap.
	const MIN_ESTIMATED_HEIGHT: f32 = 300_000.0;

	let doc = support::large(
		PARAGRAPH_COUNT,
		"The quick brown fox jumps over the lazy dog, again and again.",
	);
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(WIDE),
	);
	let estimated = layout.height();
	let update = layout
		.ensure(&doc, &mut measurer, 0.0, VIEWPORT)
		.expect("measured");

	assert_eq!(measurer.calls(), 1);
	assert_eq!(update.measurer_calls, 1);
	assert!(
		update.laid_out < MAX_LAID_OUT,
		"laid out {} blocks",
		update.laid_out
	);
	assert!(!layout.is_fully_measured());
	assert!(
		estimated > MIN_ESTIMATED_HEIGHT,
		"estimated height {estimated}"
	);

	// Every paragraph repeats the same words, so the middle of the document
	// lays out from the segment cache without calling the measurer again.
	let middle = layout.height() / 2.0;
	let update = layout
		.ensure(&doc, &mut measurer, middle, middle + VIEWPORT)
		.expect("measured");
	assert!(update.laid_out > 0);
	assert_eq!(measurer.calls(), 1);

	let hit = layout
		.hit_test(ADVANCE / 2.0, middle + 1.0)
		.expect("a block is there");
	assert!(layout.caret_rect(hit).is_some());
}

#[test]
fn measuring_above_the_viewport_reports_a_scroll_shift() {
	const PARAGRAPH_COUNT: usize = 200;
	/// Narrow enough that "short" wraps.
	const WIDTH: f32 = 20.0;
	/// The viewport height, also used as the overscan above and below it.
	const SCREEN: f32 = 400.0;
	/// Well into the document, so there are estimated blocks above.
	const VIEWPORT_TOP: f32 = 2_000.0;

	let doc = support::large(PARAGRAPH_COUNT, "short");
	let mut measurer = FixedMeasurer::new();
	let options = LayoutOptions::new(WIDTH).overscan(SCREEN);
	let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), options);

	// Estimates assume half-em characters, so "short" should take two lines
	// at 20 px; it really takes three, so measuring the overscan above the
	// viewport pushes the viewport's first block down.
	let update = layout
		.ensure(&doc, &mut measurer, VIEWPORT_TOP, VIEWPORT_TOP + SCREEN)
		.expect("measured");

	assert!(update.shift > 0.0, "shift {}", update.shift);
	assert!(update.moved_from.is_some());
}

#[test]
fn incremental_relayout_touches_one_block() {
	const WIDTH: f32 = 300.0;
	const VIEWPORT: f32 = 1_000.0;

	let mut doc = paragraphs(&["one", "two", "three"]);
	let theme = Arc::new(Theme::starter_kit());
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(&doc, theme, LayoutOptions::new(WIDTH));
	layout.layout_all(&doc, &mut measurer).expect("measured");
	let before = layout.display_list(&DisplayOptions::viewport(0.0, VIEWPORT));
	let second = doc.children(None)[1];

	let mut transaction = doc.transaction();
	transaction
		.insert_text(Point::new(second, offset("two")), "!", MarkSet::empty())
		.expect("valid point");
	let commit = transaction.commit();
	let damage = layout.apply(&doc, &commit);
	let update = layout
		.ensure(&doc, &mut measurer, 0.0, VIEWPORT)
		.expect("measured");
	let after = layout.display_list(&DisplayOptions::viewport(0.0, VIEWPORT));

	assert_eq!(damage.rects.len(), 1);
	assert_eq!(damage.from_y, None);
	assert_eq!(update.laid_out, 1);
	assert_eq!(update.measurer_calls, 1);
	assert_eq!(layout.dump(), support::layout(&doc, WIDTH).dump());

	let changed = after.damage(&before);
	let rect = layout.block_rect(second).expect("block");
	assert!(!changed.is_empty());
	assert!(
		changed
			.iter()
			.all(|damage| damage.y >= rect.y && damage.bottom() <= rect.bottom())
	);
}

#[test]
fn structural_commits_keep_untouched_layouts() {
	const WIDTH: f32 = 300.0;
	const VIEWPORT: f32 = 1_000.0;

	let mut doc = paragraphs(&["one", "two", "three"]);
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(WIDTH),
	);
	layout.layout_all(&doc, &mut measurer).expect("measured");
	let first = doc.children(None)[0];
	let paragraph = doc.schema().default_text_block();

	let mut transaction = doc.transaction();
	transaction
		.split(Point::new(first, 1), paragraph, Attrs::new())
		.expect("valid split");
	let commit = transaction.commit();
	let damage = layout.apply(&doc, &commit);
	let update = layout
		.ensure(&doc, &mut measurer, 0.0, VIEWPORT)
		.expect("measured");

	assert_eq!(damage.from_y, Some(0.0));
	assert_eq!(update.laid_out, 2, "the split block and the new one");
	assert_eq!(layout.dump(), support::layout(&doc, WIDTH).dump());
}

#[test]
fn width_and_theme_changes_relayout() {
	const WIDTH: f32 = 400.0;
	/// Narrow enough that the sentence wraps differently.
	const NARROWED: f32 = 100.0;

	let doc = paragraphs(&["The quick brown fox jumps over the lazy dog."]);
	let mut measurer = FixedMeasurer::new();
	let mut layout = layout(&doc, WIDTH);

	layout.set_width(&doc, NARROWED);
	layout.layout_all(&doc, &mut measurer).expect("measured");
	assert_eq!(layout.dump(), support::layout(&doc, NARROWED).dump());

	let theme = Theme::starter_kit().to_builder().tab_size(2).build();
	layout.set_theme(&doc, Arc::new(theme));
	assert!(!layout.is_fully_measured());
}

#[test]
fn measurer_contract_violations_are_errors() {
	struct Silent;

	impl TextMeasurer for Silent {
		fn measure(&mut self, _: &MeasureBatch<'_>, _: &mut MeasureOutput) {}
	}

	let doc = paragraphs(&["text"]);
	let mut layout = DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(100.0),
	);

	assert!(matches!(
		layout.layout_all(&doc, &mut Silent),
		Err(LayoutError::MeasurerContract { .. })
	));
}

#[test]
fn stale_layouts_report_out_of_sync() {
	let doc = paragraphs(&["text"]);
	let other = paragraphs(&["a", "b"]);
	let mut layout = DocumentLayout::new(
		&other,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(100.0),
	);

	assert!(matches!(
		layout.layout_all(&doc, &mut FixedMeasurer::new()),
		Err(LayoutError::OutOfSync(_))
	));
}

#[test]
fn display_lists_carry_selection_and_caret() {
	let hello = "hello";
	let doc = paragraphs(&["hello world"]);
	let layout = layout(&doc, WIDE);
	let block = doc.children(None)[0];
	let end_of_hello = Point::new(block, offset(hello));
	let options = DisplayOptions::viewport(0.0, 100.0)
		.selection(Selection::text(Point::new(block, 0), end_of_hello))
		.caret(Caret::new(end_of_hello));
	let list = layout.display_list(&options);
	let described = support::describe(&list);

	assert!(
		described.contains("rect Selection 0 0 48x24"),
		"{described}"
	);
	assert!(
		described
			.trim_end()
			.ends_with("rect Caret 47 0 2x24 #2e2b29"),
		"{described}"
	);
}

#[test]
fn selection_bounds_cover_every_rect() {
	let doc = paragraphs(&["aaa bbb ccc", "dd"]);
	let layout = layout(&doc, NARROW);
	let [first, second] = [doc.children(None)[0], doc.children(None)[1]];
	let selection = Selection::text(Point::new(first, 4), Point::new(second, 1));
	let bounds = layout
		.selection_bounds(&selection)
		.expect("a range selection has bounds");

	for rect in layout.selection_rects(&selection) {
		assert_eq!(bounds.union(&rect), bounds);
	}

	assert_eq!(
		layout.selection_bounds(&Selection::caret(Point::new(first, 2))),
		None
	);
}

#[test]
fn forgetting_measurements_measures_again() {
	let doc = paragraphs(&["text measured with a fallback font"]);
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(WIDE),
	);
	layout.layout_all(&doc, &mut measurer).expect("measured");
	let before = layout.dump();

	layout.forget_measurements();
	assert!(!layout.is_fully_measured());

	layout.layout_all(&doc, &mut measurer).expect("measured");
	assert_eq!(measurer.calls(), 2);
	assert_eq!(layout.dump(), before);
}
