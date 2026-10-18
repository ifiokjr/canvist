//! Layout answering the editor engine's line navigation through its trait.

#![cfg(feature = "editor")]

mod support;

use canvist_editor::Affinity;
use canvist_editor::Caret;
use canvist_editor::LineEdge;
use canvist_editor::LineNavigator;
use canvist_editor::VerticalDirection;
use canvist_layout::FixedMeasurer;
use canvist_model::Point;
use support::paragraphs;

/// Wide enough for "aaa bbb " but not the word after it.
const NARROW: f32 = 70.0;
/// Where "aaa bbb " wraps into "ccc".
const WRAP: u32 = 8;

#[test]
fn moves_down_through_the_editor_trait() {
	let doc = paragraphs(&["aaa bbb ccc", "dd"]);
	let mut layout = support::layout(&doc, NARROW);
	let [first, second] = [doc.children(None)[0], doc.children(None)[1]];
	let mut measurer = FixedMeasurer::new();
	let mut layout_navigator = layout.navigator(&doc, &mut measurer);
	let navigator: &mut dyn LineNavigator = &mut layout_navigator;
	let start = Caret::new(Point::new(first, 1));

	let wrapped = navigator
		.vertical(&doc, start, VerticalDirection::Down, None)
		.expect("laid out")
		.expect("a line below");
	assert_eq!(wrapped.caret.point, Point::new(first, WRAP + 1));

	let below = navigator
		.vertical(
			&doc,
			wrapped.caret,
			VerticalDirection::Down,
			Some(wrapped.goal_x),
		)
		.expect("laid out")
		.expect("a line below");
	assert_eq!(below.caret.point, Point::new(second, 1));
	assert_eq!(below.goal_x.to_bits(), wrapped.goal_x.to_bits());

	let last = navigator.vertical(&doc, below.caret, VerticalDirection::Down, None);
	assert_eq!(last, Ok(None));
}

#[test]
fn line_end_at_a_wrap_is_upstream() {
	let doc = paragraphs(&["aaa bbb ccc"]);
	let mut layout = support::layout(&doc, NARROW);
	let block = doc.children(None)[0];
	let mut measurer = FixedMeasurer::new();
	let mut navigator = layout.navigator(&doc, &mut measurer);
	let at = Caret::new(Point::new(block, 1));

	let end = LineNavigator::line_edge(&mut navigator, &doc, at, LineEdge::End).expect("laid out");
	assert_eq!(
		end,
		Caret {
			point: Point::new(block, WRAP),
			affinity: Affinity::Upstream,
		}
	);

	let start =
		LineNavigator::line_edge(&mut navigator, &doc, end, LineEdge::Start).expect("laid out");
	assert_eq!(start, Caret::new(Point::new(block, 0)));
}
