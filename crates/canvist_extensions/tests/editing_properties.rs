//! Property tests for the editing helpers beyond the StarterKit: random
//! alignment commands over random multi-block selections round-trip and
//! undo, typography never breaks typing, and a character limit is never
//! exceeded by local edits.

#![cfg(feature = "full")]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_editor::Extension;
use canvist_extensions::CharacterCount;
use canvist_extensions::StarterKit;
use canvist_extensions::TextAlign;
use canvist_extensions::Typography;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Value;
use proptest::prelude::*;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;
use support::assert_valid;

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

fn editor(extra: Vec<Arc<dyn Extension>>, json: Option<&Json>) -> Editor {
	let builder = Editor::builder()
		.extensions(StarterKit::default().extensions())
		.extensions(extra)
		.client(1);

	match json {
		Some(json) => {
			builder
				.build_with(|schema, client| Ok(Document::from_json(schema, client, json)?))
				.expect("the document loads")
		}
		None => builder.build().expect("builds"),
	}
}

fn aligning(json: Option<&Json>) -> Editor {
	editor(vec![Arc::new(TextAlign::default())], json)
}

/// A document of headings and paragraphs, each holding one word.
fn blocks() -> impl Strategy<Value = Json> {
	prop::collection::vec((any::<bool>(), "[a-z]{1,6}"), 1..6).prop_map(|blocks| {
		let content: Vec<Json> = blocks
			.into_iter()
			.map(|(heading, text)| {
				if heading {
					json!({ "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": text }] })
				} else {
					json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
				}
			})
			.collect();
		json!({ "type": "doc", "content": content })
	})
}

fn alignment_op() -> impl Strategy<Value = (&'static str, Option<&'static str>)> {
	let alignment = prop::sample::select(vec!["left", "center", "right", "justify"]);
	prop_oneof![
		alignment.clone().prop_map(|a| ("setTextAlign", Some(a))),
		alignment.prop_map(|a| ("toggleTextAlign", Some(a))),
		Just(("unsetTextAlign", None)),
	]
}

/// Select from block `from` to block `to` (by index), whole blocks.
fn select_blocks(editor: &mut Editor, from: usize, to: usize) {
	let doc = editor.state().doc();
	let ids = doc.children(None).to_vec();
	let (start, end) = (ids[from.min(to)], ids[from.max(to)]);
	let end_len = doc.inline(end).map_or(0, canvist_model::InlineText::len);
	let selection = Selection::Text {
		anchor: Point::new(start, 0),
		head: Point::new(end, end_len),
	};
	editor.select(selection, &mut env()).expect("selects");
}

fn whole_slice(editor: &Editor) -> canvist_editor::Slice {
	let mut copy = Typist::with(aligning(Some(&editor.state().doc().to_json())));
	copy.type_script("[Mod-a]");
	copy.editor.copy().expect("copy")
}

/// Typed characters, weighted toward the ones typography rules use.
fn keystroke() -> impl Strategy<Value = &'static str> {
	prop::sample::select(vec![
		"a", "b", "1", "2", "3", "4", " ", "-", "-", ".", "\"", "'", "<", ">", "(", ")", "c", "t",
		"m", "r", "s", "/", "+", "!", "=", "x", "*", "^", "←", "⌫", "⏎",
	])
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(96))]

	#[test]
	fn alignment_keeps_invariants_round_trips_and_undoes(
		doc in blocks(),
		steps in prop::collection::vec((alignment_op(), any::<prop::sample::Index>(), any::<prop::sample::Index>()), 1..10),
	) {
		let mut editor = aligning(Some(&doc));
		let initial = editor.state().doc().to_json();
		let count = editor.state().doc().children(None).len();

		for ((name, alignment), from, to) in &steps {
			select_blocks(&mut editor, from.index(count), to.index(count));
			let args: Attrs = alignment
				.iter()
				.map(|alignment| ("alignment", Value::from(*alignment)))
				.collect();
			editor.run_named(name, &args, &mut env()).expect("aligns");
			assert_valid(editor.state());

			let shared = editor.query("textAlign", &Attrs::new()).expect("answers");
			if *name == "setTextAlign" {
				prop_assert_eq!(shared, Value::from(alignment.unwrap_or_default()));
			}
		}

		let json = editor.state().doc().to_json();
		prop_assert_eq!(aligning(Some(&json)).state().doc().to_json(), json.clone(), "JSON");

		let schema = editor.schema().clone();
		let html = clipboard::to_html(&schema, &whole_slice(&editor));
		prop_assert_eq!(clipboard::to_html(&schema, &clipboard::from_html(&schema, &html)), html, "HTML");

		while editor.undo(&mut env()).expect("undo applies").is_some() {}
		let undone = editor.state().doc().to_json();
		for (block, original) in undone["content"].as_array().into_iter().flatten().zip(initial["content"].as_array().into_iter().flatten()) {
			let align = |node: &Json| node["attrs"]["textAlign"].as_str().map(str::to_owned);
			prop_assert_eq!(align(block), align(original), "undo restores every alignment");
			prop_assert_eq!(&block["content"], &original["content"]);
		}
	}

	#[test]
	fn typography_keeps_the_document_valid_and_undoes_to_empty(
		keys in prop::collection::vec(keystroke(), 1..40),
	) {
		let mut typist = Typist::with(editor(vec![Arc::new(Typography::default())], None));
		let empty = typist.editor.state().doc().to_json();

		for key in &keys {
			typist.type_script(key);
			assert_valid(typist.editor.state());
		}

		typist.unwind(false);
		prop_assert_eq!(typist.editor.state().doc().to_json(), empty);
	}

	#[test]
	fn typography_with_every_rule_off_types_exactly(
		keys in prop::collection::vec(keystroke().prop_filter("no editing keys or StarterKit emphasis", |key| !matches!(*key, "←" | "⌫" | "⏎" | "*")), 1..30),
	) {
		let every: Vec<&'static str> = Typography::default().rule_names().collect();
		let mut typist = Typist::with(editor(vec![Arc::new(Typography::default().without(&every))], None));
		// A leading letter keeps the StarterKit's block rules, such as `> `, out of it.
		let script = format!("a{}", keys.concat());
		typist.type_script(&script);

		let schema = typist.editor.schema().clone();
		typist.type_script("[Mod-a]");
		let slice = typist.editor.copy().expect("copy");
		prop_assert_eq!(clipboard::to_plain_text(&schema, &slice), script);
	}

	#[test]
	fn local_edits_never_exceed_the_character_limit(
		limit in 0u32..12,
		actions in prop::collection::vec(prop_oneof![
			3 => keystroke().prop_map(|key| (false, key.to_owned())),
			1 => "[a-z ]{1,8}".prop_map(|text| (true, text)),
		], 1..30),
	) {
		let mut typist = Typist::with(editor(vec![Arc::new(CharacterCount { limit: Some(limit) })], None));

		for (paste, text) in &actions {
			if *paste {
				typist.paste(text);
			} else {
				typist.type_script(text);
			}

			let characters = typist.editor.query("characters", &Attrs::new()).expect("counts");
			prop_assert!(characters.as_int().is_some_and(|count| count <= i64::from(limit)), "{characters:?} > {limit}");
		}
	}
}
