//! Property tests for mentions, hashtags, and emoji: the suggestion finder
//! agrees with a reference model, every shortcode in the table types, and
//! documents full of atoms survive JSON, HTML, Markdown, and history.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::Slice;
use canvist_extensions::Emoji;
use canvist_extensions::Hashtag;
use canvist_extensions::Mention;
use canvist_extensions::StarterKit;
use canvist_extensions::clipboard;
use canvist_extensions::emoji;
use canvist_model::Attrs;
use canvist_model::Document;
use canvist_model::Value;
use proptest::prelude::*;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

/// How far back the suggestion finder looks, in bytes, as documented on
/// `suggestion::MAX_SUGGESTION`.
const LOOKBACK: usize = 64;

/// A StarterKit editor with mentions, hashtags, and emoji, and nothing that
/// rewrites typed text, such as Typography.
fn editor() -> Editor {
	let mut builder = Editor::builder()
		.extensions(StarterKit::default().extensions())
		.client(1);
	builder.add(Mention::default());
	builder.add(Hashtag);
	builder.add(Emoji);
	builder.build().expect("the extensions build")
}

fn env(seconds: u64) -> Env<'static> {
	Env::headless(Duration::from_secs(seconds))
}

fn mention_suggestion(editor: &Editor) -> Json {
	let value = editor
		.query("mentionSuggestion", &Attrs::new())
		.expect("a known query");
	canvist_core::value_to_json(&value)
}

/// The query the finder should report for a caret after `text`: the text
/// after the last `@` within the lookback, when white space or the line
/// start comes before that `@` and no white space after it.
fn expected_query(text: &str) -> Option<String> {
	let mut start = text.len().saturating_sub(LOOKBACK);

	while !text.is_char_boundary(start) {
		start += 1;
	}

	let before = &text[start..];
	let at = before.rfind('@')?;
	let query = &before[at + 1..];
	let prefix_ok = before[..at]
		.chars()
		.next_back()
		.is_none_or(char::is_whitespace);
	let query_ok = !query.chars().any(char::is_whitespace);

	(prefix_ok && query_ok).then(|| query.to_owned())
}

fn every_shortcode() -> Vec<&'static str> {
	emoji::search("", usize::MAX)
		.into_iter()
		.map(|item| item.name)
		.collect()
}

/// The whole document as a slice, for the clipboard writers.
fn whole(doc: &Document) -> Slice {
	let blocks = doc
		.children(None)
		.iter()
		.filter_map(|&id| doc.tree(id))
		.collect();
	Slice::new(blocks)
}

/// One piece of a paragraph.
#[derive(Clone, Debug)]
enum Piece {
	Word(String),
	Mention { id: String, label: Option<String> },
	Emoji(&'static str),
	Hashtag(String),
}

fn piece() -> impl Strategy<Value = Piece> {
	let shortcodes = every_shortcode();

	prop_oneof![
		3 => "[a-zA-Z0-9]{1,8}".prop_map(Piece::Word),
		1 => ("[a-z0-9-]{1,6}", proptest::option::of("[A-Z][a-z]{0,6}"))
			.prop_map(|(id, label)| Piece::Mention { id, label }),
		1 => prop::sample::select(shortcodes).prop_map(Piece::Emoji),
		1 => "[a-z][a-z0-9_]{0,7}".prop_map(Piece::Hashtag),
	]
}

/// A paragraph of pieces separated by single spaces, as ProseMirror JSON
/// inline content, ending with a word so no export trims a trailing space.
fn paragraph(pieces: &[Piece]) -> Json {
	let mut content = Vec::new();

	for piece in pieces {
		content.push(match piece {
			Piece::Word(word) => json!({ "type": "text", "text": word }),
			Piece::Mention { id, label } => {
				let mut attrs = json!({ "id": id, "mentionSuggestionChar": "@" });

				if let Some(label) = label {
					attrs["label"] = json!(label);
				}

				json!({ "type": "mention", "attrs": attrs })
			}
			Piece::Emoji(name) => {
				let character = emoji::lookup(name).expect("a known shortcode");
				json!({ "type": "emoji", "attrs": { "name": name, "emoji": character } })
			}
			Piece::Hashtag(tag) => {
				json!({ "type": "text", "text": format!("#{tag}"), "marks": [{ "type": "hashtag", "attrs": { "tag": tag } }] })
			}
		});
		content.push(json!({ "type": "text", "text": " " }));
	}

	content.push(json!({ "type": "text", "text": "end" }));
	json!({ "type": "doc", "content": [{ "type": "paragraph", "content": content }] })
}

/// A document's first paragraph's inline content, normalised by a round
/// trip through the model so adjacent text nodes are merged.
fn inline(doc: &Document) -> Json {
	doc.to_json()["content"][0]["content"].clone()
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(128))]

	#[test]
	fn the_suggestion_finder_matches_the_model(
		pieces in prop::collection::vec(
			prop::sample::select(vec!["a", "Z", " ", "@", "é", "😀", "#", ":", "xyzxyzxyzxyz"]),
			0..40,
		),
	) {
		let text: String = pieces.concat();
		let mut typist = Typist::with(editor());
		typist.paste(&text);
		let found = mention_suggestion(&typist.editor);

		let expected = expected_query(&text).map_or(Json::Null, |query| json!({ "char": "@", "query": query }));
		prop_assert_eq!(found, expected, "after {:?}", text);
	}

	#[test]
	fn every_shortcode_types_as_its_emoji_and_backspace_restores_it(
		name in prop::sample::select(every_shortcode()),
	) {
		let mut typist = Typist::with(editor());
		typist.type_script(&format!("a :{name}:"));
		let content = typist.snapshot()["doc"][0]["content"].clone();

		prop_assert_eq!(&content[1]["type"], "emoji");
		prop_assert_eq!(&content[1]["attrs"]["name"], name);
		prop_assert_eq!(content[1]["attrs"]["emoji"].as_str(), emoji::lookup(name));

		typist.type_script("⌫");
		prop_assert_eq!(
			&typist.snapshot()["doc"][0]["content"],
			&json!([{ "type": "text", "text": format!("a :{name}:") }])
		);
	}

	#[test]
	fn search_results_are_bounded_matching_and_prefixes_first(
		query in "[a-zA-Z_+-]{0,4}",
		limit in 0_usize..40,
	) {
		let found = emoji::search(&query, limit);
		let lower = query.to_ascii_lowercase();

		prop_assert!(found.len() <= limit);
		prop_assert!(found.iter().all(|item| item.name.contains(lower.as_str())));
		let prefixed = found.iter().take_while(|item| item.name.starts_with(lower.as_str())).count();
		prop_assert!(
			found[prefixed..].iter().all(|item| !item.name.starts_with(lower.as_str())),
			"prefix matches come first"
		);
		prop_assert!(found[..prefixed].is_sorted_by_key(|item| item.name));
		prop_assert!(found[prefixed..].is_sorted_by_key(|item| item.name));
		prop_assert!(found.iter().all(|item| emoji::lookup(item.name) == Some(item.emoji)));
	}

	#[test]
	fn atoms_round_trip_through_json_html_and_markdown(pieces in prop::collection::vec(piece(), 1..12)) {
		let editor = editor();
		let schema = editor.schema().clone();
		let doc = Document::from_json(schema.clone(), 1, &paragraph(&pieces)).expect("valid JSON");

		let json = doc.to_json();
		let from_json = Document::from_json(schema.clone(), 1, &json).expect("its own JSON reads");
		prop_assert_eq!(from_json.to_json(), json.clone());

		let html = clipboard::to_html(&schema, &whole(&doc));
		let read = clipboard::from_html(&schema, &html);
		prop_assert_eq!(clipboard::to_html(&schema, &read), html.clone());
		let mut pasted = editor;
		pasted.paste(read, &mut env(0)).expect("pastes");
		prop_assert_eq!(inline(pasted.state().doc()), inline(&doc), "HTML keeps every atom: {}", html);

		let text = clipboard::to_plain_text(&schema, &whole(&doc));
		let markdown = clipboard::to_markdown(&schema, &whole(&doc));
		let reread = clipboard::from_markdown(&schema, &markdown);
		prop_assert_eq!(
			clipboard::to_plain_text(&schema, &reread),
			text,
			"Markdown has no atoms, so they come back as the text they show"
		);
		prop_assert_eq!(clipboard::to_markdown(&schema, &reread), markdown);
	}

	#[test]
	fn typed_hashtags_are_exactly_the_tags_query(tags in prop::collection::vec("[a-z][a-z0-9_]{0,7}", 1..8)) {
		let mut typist = Typist::with(editor());
		let script = tags.iter().fold(String::new(), |script, tag| script + "#" + tag + " ");
		typist.type_script(&script);

		let mut expected = tags.clone();
		expected.sort();
		expected.dedup();
		let found = canvist_core::value_to_json(
			&typist.editor.query("hashtags", &Attrs::new()).expect("a known query"),
		);
		prop_assert_eq!(found, json!(expected));
		prop_assert_eq!(
			clipboard::to_plain_text(typist.editor.schema(), &whole(typist.editor.state().doc())),
			script,
			"marking never changes the text"
		);
	}

	#[test]
	fn inserting_atoms_undoes_and_redoes_exactly(
		prefix in "[a-z ]{0,10}",
		label in proptest::option::of("[A-Z][a-z]{0,6}"),
		shortcode in prop::sample::select(every_shortcode()),
		tag in "[a-z]{1,6}",
	) {
		let mut editor = editor();
		let mut seconds = 0;
		let mut step = |editor: &mut Editor, name: &str, args: Attrs| {
			seconds += 60;
			editor.run_named(name, &args, &mut env(seconds)).expect("commands run")
		};
		editor.insert_text(&prefix, &mut env(0)).expect("typing never fails");
		let mut states = vec![editor.state().doc().to_json()];

		let label = label.map_or(Value::Null, Value::from);
		step(&mut editor, "insertMention", Attrs::new().with("id", "1").with("label", label));
		states.push(editor.state().doc().to_json());
		step(&mut editor, "setEmoji", Attrs::new().with("name", shortcode));
		states.push(editor.state().doc().to_json());
		step(&mut editor, "insertHashtag", Attrs::new().with("tag", tag.as_str()));
		states.push(editor.state().doc().to_json());

		for expected in states.iter().rev().skip(1) {
			editor.undo(&mut env(1_000)).expect("undoes");
			prop_assert_eq!(&editor.state().doc().to_json(), expected);
		}

		for expected in states.iter().skip(1) {
			editor.redo(&mut env(1_000)).expect("redoes");
			prop_assert_eq!(&editor.state().doc().to_json(), expected);
		}
	}
}
