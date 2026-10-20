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
use canvist_extensions::BackgroundColor;
use canvist_extensions::Color;
use canvist_extensions::FontFamily;
use canvist_extensions::FontSize;
use canvist_extensions::Highlight;
use canvist_extensions::HighlightHandle;
use canvist_extensions::LineHeight;
use canvist_extensions::MarkHandle;
use canvist_extensions::StarterKit;
use canvist_extensions::StyleAttrHandle;
use canvist_extensions::Subscript;
use canvist_extensions::Superscript;
use canvist_extensions::TextStyle;
use canvist_extensions::TextStyleHandle;
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
	};

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
		let answer = typist
			.editor
			.query(query.name, &Attrs::new())
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
