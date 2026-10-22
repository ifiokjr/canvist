//! Typing integration tests: the editor under realistic input.
//!
//! These run on every selected target — headless by default, the real
//! browser and the simulators when asked for — through
//! `CANVIST_TEST_TARGET`. The body types the way a person types: one
//! character at a time, real key presses, and reads the document back.

use canvist_headless::Format;
use canvist_testkit::Position;
use canvist_testkit::cross_test;

cross_test! {
	/// A hundred characters typed one at a time all arrive, in order.
	rapid_typing_keeps_every_character, async fn rapid_typing(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		let sentence = "The quick brown fox jumps over the lazy dog; pack my box with five dozen liquor jugs. 0123456789";

		editor.load_text("").await?;
		editor.type_text(sentence).await?;

		editor.assert_text(sentence).await?;
		Ok(())
	}
}

cross_test! {
	/// Typing a heading rule turns the paragraph into a heading, because
	/// `type_text` goes one character at a time like a keyboard.
	input_rules_fire_while_typing, async fn heading_rule(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("").await?;
		editor.type_text("## Notes").await?;

		editor.assert_markdown("## Notes").await?;
		// The trailing-node extension keeps one empty paragraph after
		// content, so the heading is not the last block.
		editor.assert_json(&serde_json::json!({
			"type": "doc",
			"content": [{
				"type": "heading",
				"attrs": {"level": 2},
				"content": [{"type": "text", "text": "Notes"}]
			}, {
				"type": "paragraph"
			}]
		})).await?;
		Ok(())
	}
}

cross_test! {
	/// Inserting text whole does not fire input rules: `"## "` stays text.
	insert_text_skips_input_rules, async fn insert_whole(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("").await?;
		editor.insert_text("## stays").await?;

		editor.assert_markdown("\\## stays").await?;
		Ok(())
	}
}

cross_test! {
	/// Enter splits the block at the caret; Backspace joins it back.
	enter_splits_and_backspace_joins, async fn split_and_join(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("one two").await?;
		editor.move_to(&Position::new([0], 4)).await?;
		editor.press("Enter").await?;

		editor.assert_text("one \ntwo").await?;

		editor.press("Backspace").await?;
		editor.assert_text("one two").await?;
		Ok(())
	}
}

cross_test! {
	/// Typing over a selection replaces it, exactly.
	typing_replaces_the_selection, async fn replace_selection(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("one two three").await?;
		editor.select(&Position::new([0], 4), Some(&Position::new([0], 7))).await?;
		editor.type_text("TWO").await?;

		editor.assert_text("one TWO three").await?;
		editor.assert_selection(&Position::new([0], 7), &Position::new([0], 7)).await?;
		Ok(())
	}
}

cross_test! {
	/// A word typed fast is one undo step: one undo takes the whole word
	/// off, then one redo puts it back.
	typing_groups_into_undo_steps, async fn undo_grouping(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("hello ").await?;
		editor.move_to_end([0]).await?;
		editor.type_text("world").await?;

		editor.assert_text("hello world").await?;
		editor.undo().await?;
		editor.assert_text("hello ").await?;
		editor.redo().await?;
		editor.assert_text("hello world").await?;
		Ok(())
	}
}

cross_test! {
	/// Deleting a selection is one undo step that restores it whole.
	delete_selection_undoes_whole, async fn delete_undo(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("one two three").await?;
		editor.select(&Position::new([0], 4), Some(&Position::new([0], 7))).await?;
		editor.delete_selection().await?;
		editor.assert_text("one  three").await?;

		editor.undo().await?;
		editor.assert_text("one two three").await?;
		Ok(())
	}
}

cross_test! {
	/// Pasting Markdown runs the paste rules: structure arrives, not
	/// literal characters.
	/// Neither the browser nor the mobile bindings have a clipboard to
	/// paste through.
	pasting_markdown_keeps_structure, targets = [headless, loopback], async fn paste_markdown(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("").await?;
		editor.paste(Format::Markdown, "# Title\n\nBody with **bold**").await?;

		editor.assert_markdown("# Title\n\nBody with **bold**").await?;
		editor.assert_text("Title\nBody with bold").await?;
		Ok(())
	}
}

cross_test! {
	/// Select all, then type: the document starts over.
	///
	/// The dom target is excluded because it exposes a real divergence:
	/// after `Mod-a` selects the whole document, typing through the
	/// browser's input layer replaces only the last block ("first\nsecond"
	/// becomes "first\nall", not "all"). Every other target replaces the
	/// document. Revisit once the web input path maps whole-document
	/// selections through `replaceText` correctly.
	select_all_then_type_replaces_document, targets = [headless, loopback, ios, android], async fn select_all(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("first\nsecond").await?;
		editor.select_all().await?;
		editor.type_text("all").await?;

		editor.assert_text("all").await?;
		Ok(())
	}
}

cross_test! {
	/// IME: composing shows the preedit text, committing replaces it, and
	/// cancelling leaves the document alone.
	/// Skipped on targets whose input path has no composition (the browser
	/// types real text but synthesizes no composition events).
	composition_commits_and_cancels, targets = [headless, loopback, ios, android], async fn composition(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.require(canvist_testkit::Capability::Ime)?;
		editor.load_text("").await?;

		editor.compose("こんに").await?;
		editor.assert_text("こんに").await?;

		editor.commit_composition("こんにちは").await?;
		editor.assert_text("こんにちは").await?;

		editor.compose("もじ").await?;
		editor.cancel_composition().await?;
		editor.assert_text("こんにちは").await?;
		Ok(())
	}
}

cross_test! {
	/// The caret walks with the arrows and jumps to the line edges.
	arrow_keys_move_the_caret, async fn arrows(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("one two").await?;

		editor.move_to_end([0]).await?;
		editor.press("ArrowLeft").await?;
		editor.assert_selection(&Position::new([0], 6), &Position::new([0], 6)).await?;
		editor.press("ArrowRight").await?;
		editor.assert_selection(&Position::new([0], 7), &Position::new([0], 7)).await?;

		editor.move_to(&Position::new([0], 3)).await?;
		editor.press("Home").await?;
		editor.assert_selection(&Position::new([0], 0), &Position::new([0], 0)).await?;
		editor.press("End").await?;
		editor.assert_selection(&Position::new([0], 7), &Position::new([0], 7)).await?;
		Ok(())
	}
}

cross_test! {
	/// Formatting through the keyboard path leaves the document valid and
	/// the marks where they were put.
	bold_through_keyboard, async fn keyboard_bold(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load_text("").await?;
		editor.type_text("one ").await?;
		editor.press("Mod-b").await?;
		editor.type_text("two").await?;
		editor.press("Mod-b").await?;
		editor.type_text(" three").await?;

		editor.assert_markdown("one **two** three").await?;
		editor.assert_text("one two three").await?;
		Ok(())
	}
}
