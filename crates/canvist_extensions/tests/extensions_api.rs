//! The typed API of every extension beyond the StarterKit: metadata that
//! matches the schema, commands and queries that run, typed handles that
//! agree with named commands, and the metadata snapshot that the
//! TypeScript and Dart bindings are generated from.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_editor::Command;
use canvist_editor::Editor;
use canvist_editor::EditorError;
use canvist_editor::Env;
use canvist_extensions::Alignment;
use canvist_extensions::Audio;
use canvist_extensions::BackgroundColor;
use canvist_extensions::Callout;
use canvist_extensions::CalloutHandle;
use canvist_extensions::CalloutType;
use canvist_extensions::CharacterCount;
use canvist_extensions::CharacterCountHandle;
use canvist_extensions::ClearFormatting;
use canvist_extensions::ClearFormattingHandle;
use canvist_extensions::Color;
use canvist_extensions::Comments;
use canvist_extensions::CommentsHandle;
use canvist_extensions::Details;
use canvist_extensions::DetailsHandle;
use canvist_extensions::DragHandle;
use canvist_extensions::DragHandleHandle;
use canvist_extensions::Emoji;
use canvist_extensions::EmojiHandle;
use canvist_extensions::File;
use canvist_extensions::FindAndReplace;
use canvist_extensions::FindAndReplaceHandle;
use canvist_extensions::FontFamily;
use canvist_extensions::FontSize;
use canvist_extensions::Hashtag;
use canvist_extensions::HashtagHandle;
use canvist_extensions::HeaderPart;
use canvist_extensions::Highlight;
use canvist_extensions::HighlightHandle;
use canvist_extensions::Image;
use canvist_extensions::LineHeight;
use canvist_extensions::MarkHandle;
use canvist_extensions::Mathematics;
use canvist_extensions::MathematicsHandle;
use canvist_extensions::MediaHandle;
use canvist_extensions::Mention;
use canvist_extensions::MentionHandle;
use canvist_extensions::MentionTarget;
use canvist_extensions::Placeholder;
use canvist_extensions::PlaceholderHandle;
use canvist_extensions::StarterKit;
use canvist_extensions::StyleAttrHandle;
use canvist_extensions::Subscript;
use canvist_extensions::Superscript;
use canvist_extensions::Table;
use canvist_extensions::TableHandle;
use canvist_extensions::TableOfContents;
use canvist_extensions::TableOfContentsHandle;
use canvist_extensions::TextAlign;
use canvist_extensions::TextAlignHandle;
use canvist_extensions::TextStyle;
use canvist_extensions::TextStyleHandle;
use canvist_extensions::Twitch;
use canvist_extensions::Typography;
use canvist_extensions::Youtube;
use canvist_model::Attrs;
use insta::assert_json_snapshot;
use support::Typist;

/// Handles for every extension beyond the StarterKit.
#[derive(Clone, Copy)]
struct Extras {
	subscript: MarkHandle,
	superscript: MarkHandle,
	highlight: HighlightHandle,
	text_style: TextStyleHandle,
	color: StyleAttrHandle,
	background_color: StyleAttrHandle,
	font_family: StyleAttrHandle,
	font_size: StyleAttrHandle,
	line_height: StyleAttrHandle,
	text_align: TextAlignHandle,
	character_count: CharacterCountHandle,
	placeholder: PlaceholderHandle,
	mention: MentionHandle,
	hashtag: HashtagHandle,
	emoji: EmojiHandle,
	image: MediaHandle,
	youtube: MediaHandle,
	twitch: MediaHandle,
	audio: MediaHandle,
	file: MediaHandle,
	details: DetailsHandle,
	callout: CalloutHandle,
	math: MathematicsHandle,
	table: TableHandle,
	find: FindAndReplaceHandle,
	toc: TableOfContentsHandle,
	comments: CommentsHandle,
	drag: DragHandleHandle,
	clear: ClearFormattingHandle,
}

/// The StarterKit plus every other extension, built through the typed path.
fn full() -> (Editor, Extras) {
	let mut builder = Editor::builder();
	StarterKit::add_all(&mut builder);
	let extras = Extras {
		subscript: builder.add(Subscript),
		superscript: builder.add(Superscript),
		highlight: builder.add(Highlight::default()),
		text_style: builder.add(TextStyle),
		color: builder.add(Color),
		background_color: builder.add(BackgroundColor),
		font_family: builder.add(FontFamily),
		font_size: builder.add(FontSize),
		line_height: builder.add(LineHeight),
		text_align: builder.add(TextAlign::default()),
		character_count: builder.add(CharacterCount::default()),
		placeholder: builder.add(Placeholder::default()),
		mention: builder.add(Mention::default()),
		hashtag: builder.add(Hashtag),
		emoji: builder.add(Emoji),
		image: builder.add(Image),
		youtube: builder.add(Youtube),
		twitch: builder.add(Twitch),
		audio: builder.add(Audio),
		file: builder.add(File),
		details: builder.add(Details),
		callout: builder.add(Callout),
		math: builder.add(Mathematics),
		table: builder.add(Table),
		find: builder.add(FindAndReplace),
		toc: builder.add(TableOfContents),
		comments: builder.add(Comments),
		drag: builder.add(DragHandle),
		clear: builder.add(ClearFormatting),
	};
	builder.add(Typography::default());

	(builder.build().expect("every extension builds"), extras)
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

fn typed_with(script: &str) -> (Typist, Extras) {
	let (editor, extras) = full();
	let mut typist = Typist::with(editor);
	typist.type_script(script);
	(typist, extras)
}

#[test]
fn metadata_matches_the_schema() {
	let (editor, _) = full();
	support::assert_metadata_matches_schema(&editor);
}

#[test]
fn every_command_runs_with_sample_arguments_and_rejects_missing_ones() {
	let (mut typist, _) = typed_with("# Title⏎- one⏎two⏎⏎text[Shift-ArrowLeft][Shift-ArrowLeft]");
	let metadata = typist.editor.metadata().to_vec();

	for command in metadata.iter().flat_map(|extension| &extension.commands) {
		let args: Attrs = command
			.args
			.iter()
			.map(|arg| (arg.name, arg.ty.sample()))
			.collect();
		let built = typist.editor.command_named(command.name, &args);
		let ran = built.and_then(|command| typist.editor.run(&command, &mut env()));
		assert!(ran.is_ok(), "{} runs: {ran:?}", command.name);

		if command.args.iter().any(|arg| arg.default.is_none()) {
			let missing = typist.editor.command_named(command.name, &Attrs::new());
			assert!(
				matches!(missing, Err(EditorError::InvalidArgs { .. })),
				"{} needs its arguments",
				command.name
			);
		}
	}
}

#[test]
fn every_query_answers_with_its_declared_type() {
	let (typist, _) = typed_with("plain ==marked==[Shift-ArrowLeft]");

	for query in typist
		.editor
		.metadata()
		.iter()
		.flat_map(|extension| &extension.queries)
	{
		let args: Attrs = query
			.args
			.iter()
			.map(|arg| (arg.name, arg.ty.sample()))
			.collect();
		let answer = typist
			.editor
			.query(query.name, &args)
			.expect("queries answer");
		assert!(
			query.returns.accepts(&answer),
			"{} returned {answer:?}",
			query.name
		);
	}
}

/// Run `typed` on one editor and the named command on another, starting
/// from the same script, and require the same document.
fn same_as_named(typed: fn(&Extras) -> Command, name: &str, args: &Attrs) {
	let script = "styled text[Mod-a]";
	let (mut by_handle, extras) = typed_with(script);
	let (mut by_name, _) = typed_with(script);
	by_handle
		.editor
		.run(&typed(&extras), &mut env())
		.expect("typed command");
	by_name
		.editor
		.run_named(name, args, &mut env())
		.expect("named command");

	assert_eq!(by_handle.snapshot(), by_name.snapshot(), "{name}");
}

#[test]
fn typed_handles_and_named_commands_agree() {
	let none = Attrs::new();
	same_as_named(|kit| kit.subscript.toggle(), "toggleSubscript", &none);
	same_as_named(|kit| kit.superscript.set(), "setSuperscript", &none);
	same_as_named(
		|kit| kit.highlight.set(Some("#fef08a")),
		"setHighlight",
		&Attrs::new().with("color", "#fef08a"),
	);
	same_as_named(|kit| kit.highlight.toggle(None), "toggleHighlight", &none);
	same_as_named(
		|kit| kit.text_style.remove_empty(),
		"removeEmptyTextStyle",
		&none,
	);
	same_as_named(
		|kit| kit.color.set("#958df1"),
		"setColor",
		&Attrs::new().with("color", "#958df1"),
	);
	same_as_named(
		|kit| kit.background_color.set("#fafafa"),
		"setBackgroundColor",
		&Attrs::new().with("backgroundColor", "#fafafa"),
	);
	same_as_named(
		|kit| kit.font_family.set("Georgia, serif"),
		"setFontFamily",
		&Attrs::new().with("fontFamily", "Georgia, serif"),
	);
	same_as_named(
		|kit| kit.font_size.set("20px"),
		"setFontSize",
		&Attrs::new().with("fontSize", "20px"),
	);
	same_as_named(
		|kit| kit.line_height.set("1.8"),
		"setLineHeight",
		&Attrs::new().with("lineHeight", "1.8"),
	);
	same_as_named(
		|kit| kit.text_align.set(Alignment::Center),
		"setTextAlign",
		&Attrs::new().with("alignment", "center"),
	);
	same_as_named(
		|kit| kit.text_align.toggle(Alignment::Justify),
		"toggleTextAlign",
		&Attrs::new().with("alignment", "justify"),
	);
	same_as_named(|kit| kit.text_align.unset(), "unsetTextAlign", &none);
	same_as_named(
		|kit| kit.character_count.set_limit(Some(3)),
		"setCharacterLimit",
		&Attrs::new().with("limit", 3),
	);
	same_as_named(
		|kit| kit.placeholder.set("Say hi"),
		"setPlaceholder",
		&Attrs::new().with("placeholder", "Say hi"),
	);
	same_as_named(
		|kit| {
			kit.mention.insert(MentionTarget {
				id: "7",
				label: Some("Ada"),
			})
		},
		"insertMention",
		&Attrs::new().with("id", "7").with("label", "Ada"),
	);
	same_as_named(
		|kit| kit.hashtag.insert("canvas"),
		"insertHashtag",
		&Attrs::new().with("tag", "canvas"),
	);
	same_as_named(
		|kit| kit.emoji.set("tada"),
		"setEmoji",
		&Attrs::new().with("name", "tada"),
	);
	same_as_named(
		|kit| kit.mention.dismiss(),
		"dismissMentionSuggestion",
		&none,
	);
	same_as_named(
		|kit| kit.hashtag.dismiss(),
		"dismissHashtagSuggestion",
		&none,
	);
	same_as_named(|kit| kit.emoji.dismiss(), "dismissEmojiSuggestion", &none);
	same_as_named(
		|kit| {
			kit.image
				.insert("cat.png", &Attrs::new().with("alt", "A cat"))
		},
		"setImage",
		&Attrs::new().with("src", "cat.png").with("alt", "A cat"),
	);
	same_as_named(
		|kit| {
			kit.youtube
				.insert("https://youtu.be/x", &Attrs::new().with("start", 30))
		},
		"setYoutubeVideo",
		&Attrs::new()
			.with("src", "https://youtu.be/x")
			.with("start", 30),
	);
	same_as_named(
		|kit| kit.twitch.insert("https://twitch.tv/x", &Attrs::new()),
		"setTwitchVideo",
		&Attrs::new().with("src", "https://twitch.tv/x"),
	);
	same_as_named(
		|kit| kit.audio.insert("a.mp3", &Attrs::new().with("loop", true)),
		"setAudio",
		&Attrs::new().with("src", "a.mp3").with("loop", true),
	);
	same_as_named(
		|kit| {
			kit.file
				.insert("r.pdf", &Attrs::new().with("fileName", "r.pdf"))
		},
		"insertFile",
		&Attrs::new().with("url", "r.pdf").with("fileName", "r.pdf"),
	);
	same_as_named(|kit| kit.details.set(), "setDetails", &none);
	same_as_named(
		|kit| kit.callout.toggle(CalloutType::Success),
		"toggleCallout",
		&Attrs::new().with("type", "success"),
	);
	same_as_named(
		|kit| kit.math.insert_inline("x^2"),
		"insertInlineMath",
		&Attrs::new().with("latex", "x^2"),
	);
	same_as_named(
		|kit| kit.math.insert_block("x^2"),
		"insertBlockMath",
		&Attrs::new().with("latex", "x^2"),
	);
	same_as_named(
		|kit| kit.table.insert(2, 3, false),
		"insertTable",
		&Attrs::new()
			.with("rows", 2)
			.with("cols", 3)
			.with("withHeaderRow", false),
	);
}

#[test]
fn document_tool_handles_agree_with_their_named_commands() {
	let none = Attrs::new();
	same_as_named(
		|kit| kit.find.set_term("text"),
		"setSearchTerm",
		&Attrs::new().with("term", "text"),
	);
	same_as_named(
		|kit| kit.toc.go_to(0),
		"goToHeading",
		&Attrs::new().with("index", 0),
	);
	same_as_named(
		|kit| kit.comments.set("t1"),
		"setComment",
		&Attrs::new().with("threadId", "t1"),
	);
	same_as_named(|kit| kit.drag.up(), "moveBlockUp", &none);
	same_as_named(|kit| kit.clear.clear(), "clearFormatting", &none);
}

#[test]
fn document_tool_handles_read_what_commands_set() {
	let (mut typist, kit) = typed_with("# Title⏎find me, find me");
	let editor = &mut typist.editor;

	editor
		.run(&kit.find.set_term("find"), &mut env())
		.expect("search");
	assert_eq!(kit.find.matches(editor.state()).len(), 2);
	editor.run(&kit.find.next(), &mut env()).expect("next");
	assert!(kit.find.current(editor.state()).is_some());
	editor
		.run(&kit.comments.set("t1"), &mut env())
		.expect("comment");
	assert_eq!(kit.comments.active(editor.state()), ["t1"]);
	assert_eq!(
		kit.comments.threads(editor.state()),
		[("t1".to_owned(), "find".to_owned())]
	);

	let toc = kit.toc.entries(editor.state());
	assert_eq!(
		(toc.len(), toc[0].number.as_str(), toc[0].active),
		(1, "1", true)
	);
}

#[test]
fn table_handles_edit_and_read_the_table() {
	let (mut typist, kit) = typed_with("");
	let editor = &mut typist.editor;

	assert_eq!(kit.table.cell(editor.state()), None);
	editor
		.run(&kit.table.insert(2, 2, true), &mut env())
		.expect("table");

	let cell = kit.table.cell(editor.state()).expect("in a table");
	assert_eq!((cell.row, cell.column, cell.header), (0, 0, true));
	assert_eq!((cell.rows, cell.columns), (2, 2));

	for command in [
		kit.table.add_column_after(),
		kit.table.add_row_after(),
		kit.table.go_to_next_cell(),
		kit.table.toggle_header(HeaderPart::Column),
		kit.table
			.set_cell_attribute("backgroundColor", Some("#fef08a")),
	] {
		editor.run(&command, &mut env()).expect("table command");
	}

	let cell = kit.table.cell(editor.state()).expect("in a table");
	assert_eq!((cell.rows, cell.columns, cell.column), (3, 3, 1));

	editor
		.run(&kit.table.delete_table(), &mut env())
		.expect("delete");
	assert_eq!(kit.table.cell(editor.state()), None);
}

#[test]
fn block_handles_read_their_queries() {
	let (mut typist, kit) = typed_with("note");
	let editor = &mut typist.editor;

	assert_eq!(kit.details.is_open(editor.state()), None);
	assert_eq!(kit.callout.kind(editor.state()), None);

	editor
		.run(&kit.callout.toggle(CalloutType::Warning), &mut env())
		.expect("callout");
	editor
		.run(&kit.callout.set_type(CalloutType::Error), &mut env())
		.expect("type");
	assert_eq!(kit.callout.kind(editor.state()), Some(CalloutType::Error));

	editor.run(&kit.details.set(), &mut env()).expect("details");
	assert_eq!(kit.details.is_open(editor.state()), Some(true));
	editor
		.run(&kit.details.toggle_open(), &mut env())
		.expect("toggle");
	assert_eq!(kit.details.is_open(editor.state()), Some(false));
	editor.run(&kit.details.unset(), &mut env()).expect("unset");
	assert_eq!(kit.details.is_open(editor.state()), None);
}

#[test]
fn suggestion_handles_read_the_query() {
	let (typist, kit) = typed_with("@ad");
	let state = typist.editor.state();

	assert_eq!(kit.mention.suggestion(state).as_deref(), Some("ad"));
	assert_eq!(kit.hashtag.suggestion(state), None);
	assert_eq!(kit.emoji.suggestion(state), None);
	assert!(kit.hashtag.tags(state).is_empty());
}

#[test]
fn typed_queries_read_what_commands_set() {
	let (mut typist, kit) = typed_with("ink[Mod-a]");
	let editor = &mut typist.editor;

	editor
		.run(&kit.color.set("#958df1"), &mut env())
		.expect("colour");
	editor
		.run(&kit.highlight.set(Some("#fef08a")), &mut env())
		.expect("highlight");
	editor
		.run(&kit.subscript.set(), &mut env())
		.expect("subscript");

	assert_eq!(kit.color.value(editor.state()).as_deref(), Some("#958df1"));
	assert_eq!(kit.font_size.value(editor.state()), None);
	assert_eq!(
		kit.highlight.color(editor.state()).as_deref(),
		Some("#fef08a")
	);
	assert!(kit.highlight.is_active(editor.state()));
	assert!(kit.subscript.is_active(editor.state()));
	assert!(!kit.superscript.is_active(editor.state()));

	editor
		.run(&kit.text_align.set(Alignment::Right), &mut env())
		.expect("align");
	assert_eq!(
		kit.text_align.alignment(editor.state()),
		Some(Alignment::Right)
	);
	assert_eq!(kit.character_count.characters(editor.state()), 3);
	assert_eq!(kit.character_count.words(editor.state()), 1);
	assert_eq!(kit.character_count.limit(editor.state()), None);
	assert!(
		kit.placeholder.placeholders(editor.state()).is_empty(),
		"the block has text"
	);
}

#[test]
fn style_commands_need_the_text_style_mark() {
	let mut builder = Editor::builder();
	StarterKit::add_all(&mut builder);
	let color = builder.add(Color);
	let mut editor = builder.build().expect("builds");

	assert!(matches!(
		editor.run(&color.set("red"), &mut env()),
		Err(EditorError::MissingType {
			name: "textStyle",
			..
		})
	));
}

/// The metadata of every extension, which `packages/canvist-headless` and
/// `platforms/dart` generate their typed bindings from.
#[test]
fn extension_metadata() {
	let (editor, _) = full();
	assert_json_snapshot!(editor.metadata_json());
}
