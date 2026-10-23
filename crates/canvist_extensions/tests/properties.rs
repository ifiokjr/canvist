//! Property tests over the whole StarterKit: random keystroke scripts keep
//! every invariant, and undoing everything restores the original document
//! exactly while redoing everything restores the final one.

mod support;

use canvist_extensions::clipboard;
use proptest::prelude::*;
use support::Typist;
use support::assert_valid;

/// One scripted action: a character, a key, a chord, or a paste.
fn token() -> impl Strategy<Value = String> {
	let text = prop::sample::select(vec![
		"a", "b", " ", "#", "-", "*", "_", "`", ">", "1", ".", "[[", "x", "]", "~", "é", "👍🏽", "⏸",
	]);
	let keys = prop::sample::select(vec!["⏎", "⌫", "⌦", "⇥", "⇤", "←", "→", "↑", "↓"]);
	let chords = prop::sample::select(vec![
		"[Mod-b]",
		"[Mod-i]",
		"[Mod-e]",
		"[Mod-z]",
		"[Mod-Shift-z]",
		"[Mod-a]",
		"[Mod-Shift-8]",
		"[Mod-Shift-7]",
		"[Mod-Shift-9]",
		"[Mod-Alt-1]",
		"[Mod-Alt-0]",
		"[Mod-Alt-c]",
		"[Mod-Shift-b]",
		"[Shift-Enter]",
		"[Shift-ArrowLeft]",
		"[Shift-ArrowUp]",
		"[Ctrl-Backspace]",
		"[Ctrl-Delete]",
		"[End]",
		"[Escape]",
	]);

	prop_oneof![
		8 => text.prop_map(str::to_owned),
		5 => keys.prop_map(str::to_owned),
		4 => chords.prop_map(str::to_owned),
		1 => Just("paste:one **two**\n- three".to_owned()),
	]
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(128))]

	#[test]
	fn scripts_keep_invariants_and_history_round_trips(tokens in prop::collection::vec(token(), 1..60)) {
		let mut typist = Typist::new();
		let initial = typist.editor.state().doc().to_json();

		for token in &tokens {
			match token.strip_prefix("paste:") {
				Some(text) => typist.paste(text),
				None => {
					typist.type_script(token);
				}
			}

			assert_valid(typist.editor.state());
		}

		// Scripts may end partway through undoing; redo first so the
		// history holds every change.
		typist.unwind(true);
		assert_valid(typist.editor.state());
		let last = typist.editor.state().doc().to_json();
		let undone = typist.unwind(false);
		prop_assert_eq!(typist.editor.state().doc().to_json(), initial, "undo all ({} steps)", undone);
		assert_valid(typist.editor.state());

		let redone = typist.unwind(true);
		prop_assert_eq!(redone, undone, "redo replays every step");
		prop_assert_eq!(typist.editor.state().doc().to_json(), last);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(128))]

	#[test]
	fn clipboard_formats_are_stable_under_round_trips(tokens in prop::collection::vec(token(), 1..40)) {
		let mut typist = Typist::new();

		// Literal backticks beside code spans are a known Markdown limit:
		// backslash escapes do not apply inside code spans, so `a\`` is
		// ambiguous. Code marks themselves are still exercised.
		for token in tokens.iter().filter(|token| !token.starts_with("paste:") && *token != "`") {
			typist.type_script(token);
		}

		typist.type_script("[Mod-a]");
		let schema = typist.editor.schema().clone();
		let slice = typist.editor.copy().expect("copy");

		// HTML collapses whitespace, so compare after one normalizing pass.
		let html = clipboard::to_html(&schema, &clipboard::from_html(&schema, &clipboard::to_html(&schema, &slice)));
		let from_html = clipboard::from_html(&schema, &html);
		prop_assert_eq!(clipboard::to_html(&schema, &from_html), html);

		let markdown = clipboard::to_markdown(&schema, &slice);
		let from_markdown = clipboard::from_markdown(&schema, &markdown);
		let again = clipboard::to_markdown(&schema, &from_markdown);
		prop_assert_eq!(&clipboard::to_markdown(&schema, &clipboard::from_markdown(&schema, &again)), &again);

		let mut target = Typist::new();
		target.editor.paste(from_html, &mut canvist_core::Env::headless(std::time::Duration::ZERO)).expect("paste");
		assert_valid(target.editor.state());
	}
}
