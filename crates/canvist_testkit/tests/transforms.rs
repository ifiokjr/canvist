//! Transform tests: state A to state B, quickly and synchronously.
//!
//! These never leave the process — no targets, no async — which is the
//! point: hundreds of small editor rules can live here without slowing
//! any integration suite. When a rule's input path starts to matter, the
//! same script moves up into a `cross_test!`.

use canvist_headless::Format;
use canvist_testkit::Position;
use canvist_testkit::transform::TransformCase;
use canvist_testkit::transform_test;
use serde_json::json;

transform_test! {
	/// Toggling bold on a range wraps it, and again unwraps it.
	bold_toggles, |case| case
		.text("bold me")
		.select(&Position::new([0], 0), &Position::new([0], 7))
		.press("Mod-b")
		.expect_markdown("**bold me**")
}

transform_test! {
	/// Setting a heading from a command, with its level argument.
	heading_command, |case| case
		.text("title")
		.select_all()
		.command("toggleHeading", &json!({"level": 2}))
		.expect_markdown("## title")
}

transform_test! {
	/// Backspace at a block join merges the texts on both sides.
	backspace_joins_blocks, |case| case
		.text("left\nright")
		.caret(&Position::new([1], 0))
		.press("Backspace")
		.expect_text("leftright")
}

transform_test! {
	/// Pasting plain text into a code block keeps it literal.
	code_block_paste_is_literal, |case| case
		.extensions(&["codeBlock", "history"])
		.load(Format::Markdown, "```\n```\n")
		.expect_text("")
}

transform_test! {
	/// Enter inside a paragraph splits it at the caret, keeping both
	/// halves' marks.
	enter_keeps_marks, |case| case
		.markdown("**bold** text")
		.caret(&Position::new([0], 7))
		.press("Enter")
		.expect_markdown("**bold** te\n\nxt")
}

transform_test! {
	/// A blockquote rule turns an empty paragraph into a quote and moves
	/// the caret inside.
	blockquote_rule, |case| case
		.text("")
		.type_text("> quoted")
		.expect_markdown("> quoted")
}

transform_test! {
	/// Undo after a structural change restores the exact prior document,
	/// including selection-neutral content.
	undo_restores_structure, |case| case
		.text("one")
		.caret_end([0])
		.press("Enter")
		.type_text("two")
		.undo()
		.expect_text("one\n")
}

#[test]
fn failing_cases_report_the_step() {
	let failure = TransformCase::new("reports")
		.text("abc")
		.select(&Position::new([0], 0), &Position::new([0], 1))
		.press("Mod-x-does-not-parse")
		.expect_text("never reached")
		.try_run();

	let message = failure.expect_err("the bad binding must fail");
	assert!(message.contains("`reports` step 1"), "{message}");
}
