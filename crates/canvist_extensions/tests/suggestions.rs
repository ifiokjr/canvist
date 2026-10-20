//! Suggestions and the atoms they insert: mentions, hashtags, and emoji.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Value;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

fn typist(script: &str) -> Typist {
	let mut typist = Typist::with(support::editor_with(canvist_extensions::extras()));
	typist.type_script(script);
	typist
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

fn run(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	editor
		.run_named(name, &args, &mut env())
		.expect("commands run")
		.is_some()
}

fn query(editor: &Editor, name: &str, args: &[(&'static str, Value)]) -> Json {
	let args: Attrs = args.iter().cloned().collect();
	let value = editor.query(name, &args).expect("a known query");
	canvist_editor::value_to_json(&value)
}

fn content(typist: &Typist) -> Json {
	typist.snapshot()["doc"][0]["content"].clone()
}

fn selection(typist: &Typist) -> Json {
	typist.snapshot()["selection"].clone()
}

#[test]
fn typing_a_trigger_starts_a_mention_suggestion() {
	let typist = typist("Hi @ad");

	assert_eq!(
		query(&typist.editor, "mentionSuggestion", &[]),
		json!({ "char": "@", "query": "ad" })
	);
	assert_eq!(query(&typist.editor, "emojiSuggestion", &[]), Json::Null);
}

#[test]
fn triggers_need_white_space_or_the_line_start_before_them() {
	assert_eq!(
		query(&typist("a@b").editor, "mentionSuggestion", &[]),
		Json::Null
	);
	assert_eq!(
		query(&typist("10:30").editor, "emojiSuggestion", &[]),
		Json::Null
	);
	assert_eq!(
		query(&typist("@ad min").editor, "mentionSuggestion", &[]),
		Json::Null
	);
	assert_eq!(
		query(&typist("@").editor, "mentionSuggestion", &[]),
		json!({ "char": "@", "query": "" })
	);
}

#[test]
fn inserting_a_mention_replaces_the_suggestion_and_adds_a_space() {
	let mut typist = typist("Hi @ad");

	assert!(run(
		&mut typist.editor,
		"insertMention",
		&[("id", Value::from("7")), ("label", Value::from("Ada"))]
	));
	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "Hi " },
			{ "type": "mention", "attrs": { "label": "Ada", "mentionSuggestionChar": "@" } },
			{ "type": "text", "text": " " }
		]),
		"the snapshot leaves out every `id`, the mention's too"
	);
	assert_eq!(
		selection(&typist),
		"caret 0@7",
		"after the space; an embed is three bytes"
	);
	assert_eq!(query(&typist.editor, "mentionSuggestion", &[]), Json::Null);
}

#[test]
fn backspace_after_a_mention_turns_it_back_into_its_trigger() {
	let mut typist = typist("@ad");
	run(
		&mut typist.editor,
		"insertMention",
		&[("id", Value::from("7"))],
	);

	typist.type_script("⌫⌫");
	assert_eq!(content(&typist), json!([{ "type": "text", "text": "@" }]));
	assert_eq!(
		query(&typist.editor, "mentionSuggestion", &[]),
		json!({ "char": "@", "query": "" })
	);
}

#[test]
fn escape_dismisses_until_another_trigger_is_typed() {
	let mut typist = typist("@ad[Escape]");
	assert_eq!(query(&typist.editor, "mentionSuggestion", &[]), Json::Null);

	typist.type_script("a");
	assert_eq!(
		query(&typist.editor, "mentionSuggestion", &[]),
		Json::Null,
		"the same suggestion stays dismissed"
	);

	typist.type_script(" @b");
	assert_eq!(
		query(&typist.editor, "mentionSuggestion", &[]),
		json!({ "char": "@", "query": "b" })
	);
	assert!(
		!run(&mut typist.editor, "dismissHashtagSuggestion", &[]),
		"nothing to dismiss"
	);
}

#[test]
fn code_blocks_never_suggest() {
	let typist = typist("``` @ad :smi #ta");

	assert_eq!(query(&typist.editor, "mentionSuggestion", &[]), Json::Null);
	assert_eq!(query(&typist.editor, "emojiSuggestion", &[]), Json::Null);
	assert_eq!(query(&typist.editor, "hashtagSuggestion", &[]), Json::Null);
}

#[test]
fn shortcodes_become_emoji_as_they_are_typed() {
	let typist = typist("ok :smile: and :not_an_emoji:");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "ok " },
			{ "type": "emoji", "attrs": { "emoji": "😄", "name": "smile" } },
			{ "type": "text", "text": " and :not_an_emoji:" }
		])
	);
}

#[test]
fn emoji_suggestions_search_and_insert() {
	let mut typist = typist("so :thu");

	assert_eq!(
		query(&typist.editor, "emojiSuggestion", &[]),
		json!({ "char": ":", "query": "thu" })
	);
	assert_eq!(
		query(
			&typist.editor,
			"searchEmoji",
			&[("query", Value::from("thumbs")), ("limit", Value::Int(2))]
		),
		json!([{ "name": "thumbsdown", "emoji": "👎" }, { "name": "thumbsup", "emoji": "👍" }])
	);

	assert!(run(
		&mut typist.editor,
		"setEmoji",
		&[("name", Value::from("thumbsup"))]
	));
	assert!(!run(
		&mut typist.editor,
		"setEmoji",
		&[("name", Value::from("nope"))]
	));
	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "so " },
			{ "type": "emoji", "attrs": { "emoji": "👍", "name": "thumbsup" } },
			{ "type": "text", "text": " " }
		])
	);
}

#[test]
fn hashtags_are_marked_when_the_word_ends() {
	let typist = typist("#rust and #canvas, # not");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "#rust", "marks": [{ "type": "hashtag", "attrs": { "tag": "rust" } }] },
			{ "type": "text", "text": " and " },
			{ "type": "text", "text": "#canvas", "marks": [{ "type": "hashtag", "attrs": { "tag": "canvas" } }] },
			{ "type": "text", "text": ", # not" }
		])
	);
	assert_eq!(
		query(&typist.editor, "hashtags", &[]),
		json!(["canvas", "rust"])
	);
}

#[test]
fn a_hash_and_a_space_at_the_line_start_is_still_a_heading() {
	let typist = typist("# Title");

	assert_eq!(typist.snapshot()["doc"][0]["type"], "heading");
}

#[test]
fn hashtag_suggestions_complete_with_the_mark() {
	let mut typist = typist("see #ca");

	assert_eq!(
		query(&typist.editor, "hashtagSuggestion", &[]),
		json!({ "char": "#", "query": "ca" })
	);
	assert!(run(
		&mut typist.editor,
		"insertHashtag",
		&[("tag", Value::from("#canvas"))]
	));
	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "see " },
			{ "type": "text", "text": "#canvas", "marks": [{ "type": "hashtag", "attrs": { "tag": "canvas" } }] },
			{ "type": "text", "text": " " }
		])
	);
}

#[test]
fn pasted_hashtags_are_marked() {
	let mut typist = typist("");
	typist.paste("tags: #one #two");

	assert_eq!(
		query(&typist.editor, "hashtags", &[]),
		json!(["one", "two"])
	);
}

/// A paragraph holding one of each atom, fully selected.
fn atoms() -> Typist {
	let mut typist = typist("Hi @ad");
	run(
		&mut typist.editor,
		"insertMention",
		&[("id", Value::from("7")), ("label", Value::from("Ada"))],
	);
	typist.type_script(":wave: #team![Mod-a]");
	typist
}

#[test]
fn atoms_round_trip_through_html_as_tiptap_writes_them() {
	let typist = atoms();
	let schema = typist.editor.schema();
	let slice = typist.editor.copy().expect("copy");
	let html = clipboard::to_html(schema, &slice);

	assert_eq!(
		html,
		"<p>Hi <span data-type=\"mention\" data-id=\"7\" data-label=\"Ada\" \
		 data-mention-suggestion-char=\"@\">@Ada</span> <span data-type=\"emoji\" \
		 data-name=\"wave\" data-emoji=\"👋\">👋</span> <span data-type=\"hashtag\" \
		 data-tag=\"team\">#team</span>!</p>"
	);
	assert_eq!(
		clipboard::to_html(schema, &clipboard::from_html(schema, &html)),
		html
	);
}

#[test]
fn tiptap_atoms_without_canvist_attributes_still_read() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let pasted = clipboard::from_html(
		schema,
		"<p><span data-type=\"emoji\" data-name=\"smile\">😄</span><span data-type=\"mention\" \
		 data-id=\"3\">@Grace</span></p>",
	);

	assert_eq!(clipboard::to_plain_text(schema, &pasted), "😄@Grace");
	let plain = support::editor().schema().clone();
	assert_eq!(
		clipboard::to_plain_text(
			&plain,
			&clipboard::from_html(
				&plain,
				"<p>a <span data-type=\"mention\" data-id=\"3\">@Grace</span></p>"
			)
		),
		"a @Grace",
		"without mentions, the text they showed is kept"
	);
}

#[test]
fn atoms_export_as_the_text_they_show() {
	let typist = atoms();
	let schema = typist.editor.schema();
	let slice = typist.editor.copy().expect("copy");

	assert_eq!(
		clipboard::to_plain_text(schema, &slice),
		"Hi @Ada 👋 #team!"
	);
	assert_eq!(clipboard::to_markdown(schema, &slice), "Hi @Ada 👋 #team!");
}
