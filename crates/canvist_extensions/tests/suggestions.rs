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

// Commands, queries, and keys beyond the happy path.

/// A StarterKit editor with only the mention, hashtag, and emoji
/// extensions on top, plus their typed handles.
fn atoms_editor() -> (
	Editor,
	canvist_extensions::MentionHandle,
	canvist_extensions::HashtagHandle,
	canvist_extensions::EmojiHandle,
) {
	atoms_editor_with(canvist_extensions::Mention::default())
}

fn atoms_editor_with(
	mention: canvist_extensions::Mention,
) -> (
	Editor,
	canvist_extensions::MentionHandle,
	canvist_extensions::HashtagHandle,
	canvist_extensions::EmojiHandle,
) {
	let mut builder = Editor::builder();
	builder = builder
		.extensions(canvist_extensions::StarterKit::default().extensions())
		.client(1);
	let mention = builder.add(mention);
	let hashtag = builder.add(canvist_extensions::Hashtag);
	let emoji = builder.add(canvist_extensions::Emoji);
	let editor = builder.build().expect("the extensions build");
	(editor, mention, hashtag, emoji)
}

fn invalid_args(
	result: Result<Option<canvist_editor::Update>, canvist_editor::EditorError>,
) -> String {
	match result {
		Err(canvist_editor::EditorError::InvalidArgs { name, .. }) => name,
		other => panic!("expected invalid arguments, got {other:?}"),
	}
}

#[test]
fn typed_handles_suggest_insert_and_dismiss() {
	let (editor, mention, hashtag, emoji) = atoms_editor();
	let mut typist = Typist::with(editor);
	typist.type_script("@gr");

	assert_eq!(
		mention.suggestion(typist.editor.state()).as_deref(),
		Some("gr")
	);
	assert_eq!(hashtag.suggestion(typist.editor.state()), None);
	assert_eq!(emoji.suggestion(typist.editor.state()), None);

	let target = canvist_extensions::MentionTarget {
		id: "3",
		label: Some("Grace"),
	};
	assert!(
		typist
			.editor
			.run(&mention.insert(target), &mut env())
			.expect("runs")
			.is_some()
	);
	typist.type_script("#ru");
	assert_eq!(
		hashtag.suggestion(typist.editor.state()).as_deref(),
		Some("ru")
	);
	assert!(
		typist
			.editor
			.run(&hashtag.dismiss(), &mut env())
			.expect("runs")
			.is_some()
	);
	assert_eq!(hashtag.suggestion(typist.editor.state()), None);
	assert!(
		typist
			.editor
			.run(&hashtag.insert("rust"), &mut env())
			.expect("runs")
			.is_some()
	);

	typist.type_script(":wa");
	assert_eq!(
		emoji.suggestion(typist.editor.state()).as_deref(),
		Some("wa")
	);
	assert!(
		typist
			.editor
			.run(&emoji.dismiss(), &mut env())
			.expect("runs")
			.is_some()
	);
	assert!(
		typist
			.editor
			.run(&emoji.set("not_an_emoji"), &mut env())
			.expect("runs")
			.is_none(),
		"unknown shortcodes do nothing"
	);
	assert!(
		typist
			.editor
			.run(&emoji.set("wave"), &mut env())
			.expect("runs")
			.is_some()
	);

	assert_eq!(hashtag.tags(typist.editor.state()), ["rust"]);
	assert_eq!(
		clipboard::to_plain_text(typist.editor.schema(), &whole(typist.editor.state().doc())),
		"@Grace #ru#rust :wa👋 ",
		"a dismissed suggestion keeps its text, and an insert without one goes at the caret"
	);
}

#[test]
fn a_custom_trigger_may_allow_spaces() {
	let mention = canvist_extensions::Mention {
		char: '+',
		allow_spaces: true,
	};
	let (editor, mention, ..) = atoms_editor_with(mention);
	let mut typist = Typist::with(editor);
	typist.type_script("hi +Ada Lov");

	assert_eq!(
		query(&typist.editor, "mentionSuggestion", &[]),
		json!({ "char": "+", "query": "Ada Lov" })
	);
	assert_eq!(
		mention.suggestion(typist.editor.state()).as_deref(),
		Some("Ada Lov")
	);
	assert!(run(
		&mut typist.editor,
		"insertMention",
		&[
			("id", Value::from("1")),
			("label", Value::from("Ada Lovelace"))
		]
	));
	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "hi " },
			{ "type": "mention", "attrs": { "label": "Ada Lovelace", "mentionSuggestionChar": "+" } },
			{ "type": "text", "text": " " }
		])
	);

	typist.type_script("⌫⌫");
	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "hi +" }]),
		"Backspace reopens with the mention's own trigger"
	);
}

#[test]
fn inserting_without_a_suggestion_goes_at_the_caret_and_reuses_a_following_space() {
	let mut typist = typist("a  b←←");

	assert!(run(
		&mut typist.editor,
		"insertMention",
		&[("id", Value::from("1")), ("label", Value::from("Ann"))]
	));
	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "a " },
			{ "type": "mention", "attrs": { "label": "Ann", "mentionSuggestionChar": "@" } },
			{ "type": "text", "text": " b" }
		]),
		"the space already after the caret is not doubled"
	);
	assert_eq!(
		selection(&typist),
		"caret 0@6",
		"the caret moves past that space"
	);
}

#[test]
fn mentions_keep_the_active_marks() {
	let mut typist = typist("[Mod-b]@ad");
	run(
		&mut typist.editor,
		"insertMention",
		&[("id", Value::from("7"))],
	);

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "mention", "attrs": { "label": null, "mentionSuggestionChar": "@" }, "marks": [{ "type": "bold" }] },
			{ "type": "text", "text": " ", "marks": [{ "type": "bold" }] }
		])
	);
}

#[test]
fn atom_commands_need_a_caret() {
	let mut typist = typist("Hello[Mod-a]");
	let mut env = env();

	for (name, args) in [
		("insertMention", Attrs::new().with("id", "1")),
		("insertHashtag", Attrs::new().with("tag", "a")),
		("setEmoji", Attrs::new().with("name", "smile")),
	] {
		let command = typist.editor.command_named(name, &args).expect("known");
		assert!(
			!typist.editor.can(&command, &mut env).expect("checks"),
			"{name}"
		);
	}

	typist.type_script("→");
	let command = typist
		.editor
		.command_named("insertMention", &Attrs::new().with("id", "1"))
		.expect("known");
	assert!(typist.editor.can(&command, &mut env).expect("checks"));
	assert_eq!(
		clipboard::to_plain_text(typist.editor.schema(), &whole(typist.editor.state().doc())),
		"Hello",
		"can() changes nothing"
	);
}

#[test]
fn invalid_arguments_are_reported() {
	let mut typist = typist("@a");
	let mut env = env();
	let mut named = |name: &str, args: Attrs| typist.editor.run_named(name, &args, &mut env);

	assert_eq!(
		invalid_args(named("insertMention", Attrs::new())),
		"insertMention"
	);
	assert_eq!(
		invalid_args(named("insertMention", Attrs::new().with("id", 7))),
		"insertMention"
	);
	assert_eq!(
		invalid_args(named(
			"insertMention",
			Attrs::new().with("id", "7").with("label", true)
		)),
		"insertMention"
	);
	assert_eq!(
		invalid_args(named("insertHashtag", Attrs::new())),
		"insertHashtag"
	);
	assert_eq!(
		invalid_args(named("setEmoji", Attrs::new().with("name", 1))),
		"setEmoji"
	);

	let error = typist
		.editor
		.query("searchEmoji", &Attrs::new())
		.expect_err("a query is required");
	assert!(
		matches!(error, canvist_editor::EditorError::InvalidArgs { .. }),
		"{error:?}"
	);
	let error = typist
		.editor
		.query(
			"searchEmoji",
			&Attrs::new().with("query", "a").with("limit", "ten"),
		)
		.expect_err("the limit is an int");
	assert!(
		matches!(error, canvist_editor::EditorError::InvalidArgs { .. }),
		"{error:?}"
	);
}

#[test]
fn handle_commands_fail_on_a_schema_without_their_type() {
	let (_, mention, hashtag, emoji) = atoms_editor();
	let mut plain = Typist::new();
	plain.type_script("x");
	let mut env = env();

	for command in [
		mention.insert(canvist_extensions::MentionTarget {
			id: "1",
			label: None,
		}),
		hashtag.insert("tag"),
		emoji.set("smile"),
	] {
		let error = plain
			.editor
			.run(&command, &mut env)
			.expect_err("no such type");
		assert!(
			matches!(error, canvist_editor::EditorError::MissingType { .. }),
			"{error:?}"
		);
	}

	assert_eq!(
		plain.snapshot()["doc"][0]["content"],
		json!([{ "type": "text", "text": "x" }])
	);
	assert_eq!(
		hashtag.tags(plain.editor.state()),
		Vec::<String>::new(),
		"queries answer empty without the mark"
	);
}

#[test]
fn search_emoji_honours_its_limit_and_ignores_case() {
	let editor = typist("").editor;
	let search = |args: &[(&'static str, Value)]| query(&editor, "searchEmoji", args);

	assert_eq!(
		search(&[("query", Value::from("face"))])
			.as_array()
			.map(Vec::len),
		Some(10),
		"ten unless asked"
	);
	assert_eq!(
		search(&[("query", Value::from("smile")), ("limit", Value::Int(0))]),
		json!([])
	);
	assert_eq!(
		search(&[("query", Value::from("smile")), ("limit", Value::Int(-3))]),
		json!([]),
		"a negative limit finds nothing"
	);
	assert_eq!(
		search(&[("query", Value::from("TADA")), ("limit", Value::Int(1))]),
		json!([{ "name": "tada", "emoji": "🎉" }])
	);
	assert_eq!(search(&[("query", Value::from("zzzz"))]), json!([]));
	assert_eq!(
		search(&[("query", Value::from("")), ("limit", Value::Int(2))]),
		json!([{ "name": "+1", "emoji": "👍" }, { "name": "-1", "emoji": "👎" }]),
		"an empty query lists shortcodes in order"
	);
}

#[test]
fn backspace_after_a_shortcode_restores_it() {
	let mut typist = typist("hi :smile:");
	assert_eq!(content(&typist)[1]["type"], "emoji");

	typist.type_script("⌫");
	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "hi :smile:" }])
	);
	typist.type_script("[Mod-z]");
	assert_eq!(
		content(&typist)[1]["type"],
		"emoji",
		"undo reapplies the rule"
	);
}

#[test]
fn backspace_after_a_typed_hashtag_unmarks_it() {
	let mut typist = typist("#rust ");
	assert_eq!(query(&typist.editor, "hashtags", &[]), json!(["rust"]));

	typist.type_script("⌫");
	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "#rust " }])
	);
	assert_eq!(query(&typist.editor, "hashtags", &[]), json!([]));
}

#[test]
fn hashtags_do_not_grow_and_ignore_tags_inside_words() {
	let typist = typist("a#b #ok!x #two#three ");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "a#b " },
			{ "type": "text", "text": "#ok", "marks": [{ "type": "hashtag", "attrs": { "tag": "ok" } }] },
			{ "type": "text", "text": "!x #two#three " }
		]),
		"text typed after a tag is unmarked, and a second `#` ends no tag"
	);
}

#[test]
fn every_terminator_ends_a_hashtag() {
	for end in [" ", ".", ",", ";", ":", "!", "?", ")", "]"] {
		let typist = typist(&format!("#tag{end}"));

		assert_eq!(
			query(&typist.editor, "hashtags", &[]),
			json!(["tag"]),
			"{end:?}"
		);
		assert_eq!(
			content(&typist)[1],
			json!({ "type": "text", "text": end }),
			"{end:?}"
		);
	}
}

#[test]
fn code_blocks_keep_shortcodes_and_hashtags_literal() {
	let typist = typist("``` :smile: #tag ");

	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": ":smile: #tag " }])
	);
}

#[test]
fn hashtag_commands_strip_the_hash_and_list_unique_tags() {
	let mut typist = typist("#b, #a, #b ");
	assert!(run(
		&mut typist.editor,
		"insertHashtag",
		&[("tag", Value::from("##a"))]
	));

	assert_eq!(query(&typist.editor, "hashtags", &[]), json!(["a", "b"]));
	assert_eq!(query(&typist.editor, "hashtagSuggestion", &[]), Json::Null);
}

#[test]
fn suggestions_look_back_only_a_short_way() {
	let long = "x".repeat(70);
	let typist = typist(&format!("@{long}"));

	assert_eq!(
		query(&typist.editor, "mentionSuggestion", &[]),
		Json::Null,
		"a trigger more than 64 bytes back is not a suggestion"
	);

	let typist = self::typist("é😀 @ü");
	assert_eq!(
		query(&typist.editor, "mentionSuggestion", &[]),
		json!({ "char": "@", "query": "ü" }),
		"multi-byte text before the trigger is fine"
	);
}

#[test]
fn a_range_selection_has_no_suggestion() {
	let typist = typist("@ada[Shift-ArrowLeft]");

	assert_eq!(query(&typist.editor, "mentionSuggestion", &[]), Json::Null);
}

#[test]
fn a_dismissed_suggestion_reopens_after_a_new_trigger_elsewhere() {
	let mut typist = typist(":sm[Escape]");
	assert_eq!(query(&typist.editor, "emojiSuggestion", &[]), Json::Null);

	typist.type_script("⏎:sm");
	assert_eq!(
		query(&typist.editor, "emojiSuggestion", &[]),
		json!({ "char": ":", "query": "sm" }),
		"the same offset in another block is a new suggestion"
	);
}

#[test]
fn escape_without_a_suggestion_falls_through_to_the_standard_key() {
	let mut typist = typist("plain");
	typist.type_script("[Escape]");

	assert_eq!(
		selection(&typist),
		"blocks 0 → 0",
		"Escape selects the block, as it does without these extensions"
	);
	assert!(
		!typist
			.editor
			.state()
			.settings()
			.iter()
			.any(|(key, _)| key.ends_with("Dismissed"))
	);
}

/// Run a named command a minute into the session, well after any typing,
/// so it is its own undo step.
fn run_later(editor: &mut Editor, name: &str, args: &[(&'static str, Value)], minute: u64) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	let mut env = Env::headless(Duration::from_secs(60 * minute));
	editor
		.run_named(name, &args, &mut env)
		.expect("commands run")
		.is_some()
}

#[test]
fn undo_and_redo_restore_suggestions_and_atoms() {
	let mut typist = typist("Hi @ad");
	let suggesting = typist.snapshot();
	run_later(
		&mut typist.editor,
		"insertMention",
		&[("id", Value::from("7")), ("label", Value::from("Ada"))],
		1,
	);
	let inserted = typist.snapshot();
	let mut env = Env::headless(Duration::from_secs(120));

	typist.editor.undo(&mut env).expect("undoes");
	assert_eq!(typist.snapshot(), suggesting);
	assert_eq!(
		query(&typist.editor, "mentionSuggestion", &[]),
		json!({ "char": "@", "query": "ad" }),
		"undo brings the suggestion back"
	);
	typist.editor.redo(&mut env).expect("redoes");
	assert_eq!(typist.snapshot(), inserted);

	let mut typist = self::typist("x :thu");
	let suggesting = typist.snapshot();
	run_later(
		&mut typist.editor,
		"setEmoji",
		&[("name", Value::from("thumbsup"))],
		1,
	);
	let emoji = typist.snapshot();
	run_later(
		&mut typist.editor,
		"insertHashtag",
		&[("tag", Value::from("tagged"))],
		2,
	);
	let tagged = typist.snapshot();

	typist.editor.undo(&mut env).expect("undoes");
	assert_eq!(typist.snapshot(), emoji);
	assert_eq!(query(&typist.editor, "hashtags", &[]), json!([]));
	typist.editor.undo(&mut env).expect("undoes");
	assert_eq!(typist.snapshot(), suggesting);
	typist.editor.redo(&mut env).expect("redoes");
	typist.editor.redo(&mut env).expect("redoes");
	assert_eq!(typist.snapshot(), tagged);
	assert_eq!(query(&typist.editor, "hashtags", &[]), json!(["tagged"]));
}

#[test]
fn backspace_elsewhere_still_deletes() {
	let typist = typist("ab⌫");
	assert_eq!(content(&typist), json!([{ "type": "text", "text": "a" }]));

	let mut typist = self::typist(":smile:x⌫⌫");
	assert_eq!(
		content(&typist),
		json!(null),
		"once another key is pressed, an emoji deletes as one character"
	);
	typist.type_script("@a");
	run(
		&mut typist.editor,
		"insertMention",
		&[("id", Value::from("1"))],
	);
	typist.type_script("⌫⌫⌫");
	assert_eq!(
		content(&typist),
		json!(null),
		"the trigger it became deletes too"
	);
}

// Serialisation round trips.

#[test]
fn atoms_round_trip_through_prosemirror_json() {
	let typist = atoms();
	let doc = typist.editor.state().doc();
	let json = doc.to_json();
	let restored = canvist_model::Document::from_json(typist.editor.schema().clone(), 1, &json)
		.expect("valid JSON");

	assert_eq!(restored.to_json(), json);
	let content = &json["content"][0]["content"];
	assert_eq!(content[1]["attrs"]["id"], "7", "the mention keeps its id");
	assert_eq!(
		content[3]["attrs"],
		json!({ "emoji": "👋", "name": "wave" })
	);
	assert_eq!(
		content[5]["marks"],
		json!([{ "type": "hashtag", "attrs": { "tag": "team" } }])
	);
}

#[test]
fn tiptap_json_atoms_take_their_defaults() {
	let schema = typist("").editor.schema().clone();
	let doc = canvist_model::Document::from_json(
		schema.clone(),
		1,
		&json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
			{ "type": "mention", "attrs": { "id": "u1" } },
			{ "type": "text", "text": " " },
			{ "type": "emoji", "attrs": { "name": "smile" } },
			{ "type": "text", "text": " #x", "marks": [{ "type": "hashtag", "attrs": { "tag": "x" } }] }
		] }] }),
	)
	.expect("TipTap JSON reads");
	let content = &doc.to_json()["content"][0]["content"];

	assert_eq!(content[0]["attrs"], json!({ "id": "u1" }));
	assert_eq!(content[2]["attrs"], json!({ "name": "smile" }));
	let slice = whole(&doc);
	assert_eq!(
		clipboard::to_plain_text(&schema, &slice),
		"@u1 :smile: #x",
		"a mention without a label shows its id, an emoji without its character its shortcode"
	);
}

#[test]
fn atoms_export_to_markdown_as_text_and_read_back_as_text() {
	let typist = atoms();
	let schema = typist.editor.schema();
	let markdown = clipboard::to_markdown(schema, &typist.editor.copy().expect("copy"));
	let read = clipboard::from_markdown(schema, &markdown);

	assert_eq!(clipboard::to_plain_text(schema, &read), "Hi @Ada 👋 #team!");
	assert_eq!(
		clipboard::to_markdown(schema, &read),
		markdown,
		"Markdown has no atoms, so it is stable but lossy"
	);
}

#[test]
fn pasted_html_keeps_atoms_and_hashtags() {
	let mut typist = typist("");
	let schema = typist.editor.schema().clone();
	let slice = clipboard::from_html(
		&schema,
		"<p>a <span data-type=\"mention\" data-id=\"9\" data-label=\"Zed\" \
		 data-mention-suggestion-char=\"+\">+Zed</span> <span data-type=\"emoji\" \
		 data-name=\"tada\" data-emoji=\"🎉\">🎉</span> <span data-type=\"hashtag\" \
		 data-tag=\"x\">#x</span></p>",
	);
	typist.editor.paste(slice, &mut env()).expect("pastes");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "a " },
			{ "type": "mention", "attrs": { "label": "Zed", "mentionSuggestionChar": "+" } },
			{ "type": "text", "text": " " },
			{ "type": "emoji", "attrs": { "emoji": "🎉", "name": "tada" } },
			{ "type": "text", "text": " " },
			{ "type": "text", "text": "#x", "marks": [{ "type": "hashtag", "attrs": { "tag": "x" } }] }
		])
	);
	assert_eq!(query(&typist.editor, "hashtags", &[]), json!(["x"]));
}

#[test]
fn html_escapes_atom_attributes() {
	let mut typist = typist("@a");
	run(
		&mut typist.editor,
		"insertMention",
		&[
			("id", Value::from("\"<1>\"")),
			("label", Value::from("A&B")),
		],
	);
	typist.type_script("<");
	let schema = typist.editor.schema();
	let html = clipboard::to_html(schema, &whole(typist.editor.state().doc()));

	assert_eq!(
		html,
		"<p><span data-type=\"mention\" data-id=\"&quot;&lt;1&gt;&quot;\" data-label=\"A&amp;B\" \
		 data-mention-suggestion-char=\"@\">@A&amp;B</span> &lt;</p>"
	);
	assert_eq!(
		clipboard::to_html(schema, &clipboard::from_html(schema, &html)),
		html
	);
}

/// The whole document as a slice, for the clipboard writers.
fn whole(doc: &canvist_model::Document) -> canvist_editor::Slice {
	let blocks = doc
		.children(None)
		.iter()
		.filter_map(|&id| doc.tree(id))
		.collect();
	canvist_editor::Slice::new(blocks)
}

/// Regression: reading a label-less mention back from HTML took the id it
/// shows as its label, so `{ id: "u1" }` came back as `{ id: "u1", label:
/// "u1" }`.
#[test]
fn a_mention_without_a_label_round_trips_through_html_without_gaining_one() {
	let mut typist = typist("@u");
	run(
		&mut typist.editor,
		"insertMention",
		&[("id", Value::from("u1"))],
	);
	typist.type_script("x");
	let schema = typist.editor.schema();
	let html = clipboard::to_html(schema, &whole(typist.editor.state().doc()));

	assert_eq!(
		html,
		"<p><span data-type=\"mention\" data-id=\"u1\" \
		 data-mention-suggestion-char=\"@\">@u1</span> x</p>"
	);
	let mut pasted = self::typist("");
	pasted
		.editor
		.paste(clipboard::from_html(schema, &html), &mut env())
		.expect("pastes");
	let mention = &pasted.editor.state().doc().to_json()["content"][0]["content"][0];
	assert_eq!(
		mention["attrs"],
		json!({ "id": "u1", "mentionSuggestionChar": "@" })
	);

	let older = clipboard::from_html(
		schema,
		"<p><span data-type=\"mention\" data-id=\"3\">@Grace</span></p>",
	);
	pasted.editor.paste(older, &mut env()).expect("pastes");
	let doc = pasted.editor.state().doc().to_json();
	let labels: Vec<&Json> = doc["content"][0]["content"]
		.as_array()
		.expect("inline content")
		.iter()
		.map(|node| &node["attrs"]["label"])
		.collect();
	assert_eq!(
		labels,
		[&Json::Null, &Json::Null, &json!("Grace")],
		"a label shown without `data-label` is still read"
	);
}
