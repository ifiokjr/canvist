//! The emoji table as data: without the `emoji-data` feature, `emoji`
//! knows no shortcode until `emoji::load` hands it the table, as the
//! TypeScript full module does on first use. Run with
//! `--no-default-features --features extras`.
//!
//! The table is process-wide, so one test walks through before and after.

#![cfg(all(
	feature = "emoji",
	feature = "starter-kit",
	not(feature = "emoji-data")
))]

use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_extensions::Emoji;
use canvist_extensions::StarterKit;
use canvist_extensions::emoji;
use canvist_model::Attrs;
use serde_json::json;

const TABLE: &str = include_str!("../src/emoji/gemoji.txt");

fn editor() -> Editor {
	Editor::builder()
		.extensions(StarterKit::default().extensions())
		.extension(Emoji)
		.client(1)
		.build()
		.expect("the extensions build")
}

fn typed(text: &str) -> serde_json::Value {
	let mut editor = editor();
	let mut env = Env::headless(Duration::ZERO);

	for character in text.chars() {
		editor
			.insert_text(&character.to_string(), &mut env)
			.expect("typing never fails");
	}

	editor.state().doc().to_json()["content"][0]["content"].clone()
}

fn search(editor: &Editor, query: &str) -> serde_json::Value {
	let value = editor
		.query("searchEmoji", &Attrs::new().with("query", query))
		.expect("a known query");
	canvist_core::value_to_json(&value)
}

#[test]
fn emoji_know_nothing_until_the_table_loads_and_then_everything() {
	let mut editor = editor();
	let mut env = Env::headless(Duration::ZERO);

	assert!(!emoji::is_loaded());
	assert_eq!(emoji::lookup("smile"), None);
	assert_eq!(search(&editor, "smile"), json!([]));
	assert_eq!(
		typed(":smile:"),
		json!([{ "type": "text", "text": ":smile:" }])
	);
	let set = editor
		.command_named("setEmoji", &Attrs::new().with("name", "smile"))
		.expect("known");
	assert!(
		editor.run(&set, &mut env).expect("runs").is_none(),
		"no shortcode is known yet, so nothing is inserted"
	);

	assert!(
		emoji::load(TABLE.to_owned()),
		"the first load takes the table"
	);
	assert!(!emoji::load(String::new()), "later loads are ignored");
	assert!(emoji::is_loaded());

	assert_eq!(emoji::lookup("smile"), Some("😄"));
	assert_eq!(
		search(&editor, "tada"),
		json!([{ "name": "tada", "emoji": "🎉" }])
	);
	assert_eq!(
		typed(":smile:"),
		json!([{ "type": "emoji", "attrs": { "emoji": "😄", "name": "smile" } }])
	);
	assert!(editor.run(&set, &mut env).expect("runs").is_some());
}
