//! Extension interaction tests: extensions in combination, which is where
//! they break each other.
//!
//! Every test states its extension set through `config`, the way an app
//! picks what it ships, and the same set is registered on every target.

use canvist_headless::Format;
use canvist_testkit::EditorConfig;
use canvist_testkit::Position;
use canvist_testkit::cross_test;
use serde_json::json;

cross_test! {
	/// Headings, lists, and marks together: a document that uses them all
	/// at once exports with every piece intact.
	starter_kit_extensions_coexist, async fn everything_at_once(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("").await?;

		editor.type_text("# Release").await?;
		editor.press("Enter").await?;
		editor.type_text("- ship ").await?;
		editor.press("Mod-b").await?;
		editor.type_text("the editor").await?;
		editor.press("Mod-b").await?;
		editor.press("Enter").await?;
		editor.press("Tab").await?;
		editor.type_text("nested point").await?;

		editor.assert_markdown("# Release\n\n- ship **the editor**\n  - nested point").await?;
		Ok(())
	}
}

cross_test! {
	/// Two marks on one word: whichever toggles second does not undo the
	/// first.
	two_marks_stack, config = |config: &mut EditorConfig| *config = EditorConfig::with_extensions(&["bold", "italic", "history"]), async fn stacking(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("").await?;

		editor.type_text("word").await?;
		editor.select_all().await?;
		editor.run_command("toggleBold", &json!(null)).await?;
		editor.run_command("toggleItalic", &json!(null)).await?;

		editor.assert_markdown("***word***").await?;
		Ok(())
	}
}

cross_test! {
	/// A code block and a list next to each other keep their own rules:
	/// typing into the list after the code block stays out of it, and
	/// each exports as its own structure.
	code_block_and_list_together, config = |config: &mut EditorConfig| *config = EditorConfig::with_extensions(&["codeBlock", "bulletList", "listItem", "history"]), async fn neighbors(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load(Format::Markdown, "```\nx = 1\n```\n\n- item").await?;

		editor.move_to_end([1, 0, 0]).await?;
		editor.type_text(" two").await?;

		editor.assert_markdown("```\nx = 1\n```\n\n- item two").await?;

		editor.move_to_end([0]).await?;
		editor.type_text("\ny = 2").await?;
		editor.assert_markdown("```\nx = 1\ny = 2\n```\n\n- item two").await?;
		Ok(())
	}
}

cross_test! {
	/// A command an extension did not register fails cleanly, on every
	/// target, without touching the document.
	unregistered_command_fails_cleanly, config = |config: &mut EditorConfig| *config = EditorConfig::with_extensions(&["bold", "history"]), async fn missing_command(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("keep").await?;

		let before = editor.markdown().await?;
		let result = editor.run_command("toggleItalic", &json!(null)).await;

		assert!(result.is_err(), "toggleItalic applied without the extension");
		editor.assert_markdown(&before).await?;
		Ok(())
	}
}

cross_test! {
	/// One extension's query reports what another's command did: the
	/// registry agrees across extensions.
	queries_see_other_extensions_marks, config = |config: &mut EditorConfig| *config = EditorConfig::with_extensions(&["bold", "italic", "history"]), async fn cross_query(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("word").await?;
		editor.select_all().await?;
		editor.run_command("toggleBold", &json!(null)).await?;

		let bold = editor.query("isBold", &json!(null)).await?;
		let italic = editor.query("isItalic", &json!(null)).await?;

		assert_eq!(bold, json!(true));
		assert_eq!(italic, json!(false));
		Ok(())
	}
}

cross_test! {
	/// Task lists, headings, and links in one document: the extras set,
	/// together, still types and exports.
	extras_extensions_together, targets = [headless, loopback], config = |config: &mut EditorConfig| *config = EditorConfig::with_extensions(&[
		"heading", "taskList", "listItem", "link", "history", "trailingNode",
	]), async fn tasks_and_links(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("").await?;
		editor.type_text("## Todo").await?;
		editor.press("Enter").await?;
		editor.type_text("[ ] write tests").await?;

		editor.assert_markdown("## Todo\n\n- [ ] write tests").await?;
		Ok(())
	}
}

cross_test! {
	/// Undo covers what several extensions did together: heading, bold,
	/// and a list collapse in the order they were made.
	undo_across_extensions, async fn undo_everything(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("").await?;
		editor.type_text("plain").await?;
		editor.select_all().await?;
		editor.run_command("toggleHeading", &json!({"level": 3})).await?;
		editor.assert_markdown("### plain").await?;

		editor.run_command("undo", &json!(null)).await?;
		editor.assert_markdown("plain").await?;
		Ok(())
	}
}

cross_test! {
	/// A mark applied to a selection spanning two blocks lands in both,
	/// and only on the selected range.
	mark_across_blocks, async fn across_blocks(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("first\nsecond").await?;
		editor.select(&Position::new([0], 2), Some(&Position::new([1], 3))).await?;
		editor.press("Mod-b").await?;

		editor.assert_markdown("fi**rst**\n\n**sec**ond").await?;
		Ok(())
	}
}

cross_test! {
	/// Loading every format the editor ships round-trips through text:
	/// whatever came in, the plain text export agrees.
	every_format_loads, async fn formats(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load(Format::Markdown, "# Head\n\npara **bold**").await?;
		editor.assert_text("Head\npara bold").await?;

		editor.load(Format::Html, "<h2>Two</h2><p>body</p>").await?;
		editor.assert_markdown("## Two\n\nbody").await?;

		editor.load(Format::Text, "a\nb").await?;
		editor.assert_text("a\nb").await?;
		Ok(())
	}
}
