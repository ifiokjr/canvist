//! Property tests for the formatting extensions beyond the StarterKit:
//! random sequences of subscript, superscript, highlight, and text style
//! commands over random ranges keep the document valid, round-trip through
//! HTML, Markdown, and ProseMirror JSON, and undo back to the start.

#![cfg(all(
	feature = "clipboard",
	feature = "highlight",
	feature = "subscript",
	feature = "superscript",
	feature = "text-style-kit"
))]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_extensions::BackgroundColor;
use canvist_extensions::Color;
use canvist_extensions::FontFamily;
use canvist_extensions::FontSize;
use canvist_extensions::Highlight;
use canvist_extensions::LineHeight;
use canvist_extensions::StarterKit;
use canvist_extensions::Subscript;
use canvist_extensions::Superscript;
use canvist_extensions::TextStyle;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Value;
use proptest::prelude::*;
use serde_json::Value as Json;
use support::assert_valid;

fn editor_from(json: Option<&Json>) -> Editor {
	let extras: Vec<Arc<dyn Extension>> = vec![
		Arc::new(Subscript),
		Arc::new(Superscript),
		Arc::new(Highlight::default()),
		Arc::new(TextStyle),
		Arc::new(Color),
		Arc::new(BackgroundColor),
		Arc::new(FontFamily),
		Arc::new(FontSize),
		Arc::new(LineHeight),
	];
	let builder = Editor::builder()
		.extensions(StarterKit::default().extensions())
		.extensions(extras)
		.client(1);

	match json {
		Some(json) => {
			builder
				.build_with(|schema, client| Ok(Document::from_json(schema, client, json)?))
				.expect("the document loads")
		}
		None => builder.build().expect("the extensions build"),
	}
}

/// An editor holding `text` in one paragraph, with nothing in its history.
fn typed(text: &str) -> Editor {
	editor_from(Some(&serde_json::json!({
		"type": "doc",
		"content": [{ "type": "paragraph", "content": [{ "type": "text", "text": text }] }]
	})))
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

/// One formatting command with its arguments.
#[derive(Clone, Debug)]
struct Op {
	name: &'static str,
	args: Vec<(&'static str, Value)>,
}

fn op() -> impl Strategy<Value = Op> {
	let colour = prop::sample::select(vec!["#fef08a", "#fbcfe8", "#958df1"]);
	let size = prop::sample::select(vec!["12px", "18px", "2rem"]);
	let family = prop::sample::select(vec!["Georgia", "Inter, sans-serif"]);
	let simple = prop::sample::select(vec![
		"toggleSubscript",
		"setSubscript",
		"unsetSubscript",
		"toggleSuperscript",
		"setSuperscript",
		"unsetSuperscript",
		"unsetHighlight",
		"toggleBold",
		"unsetColor",
		"unsetBackgroundColor",
		"unsetFontFamily",
		"unsetFontSize",
		"unsetLineHeight",
		"removeEmptyTextStyle",
	]);
	let with = |name: &'static str, key: &'static str| {
		move |value: &str| {
			Op {
				name,
				args: vec![(key, Value::from(value))],
			}
		}
	};

	prop_oneof![
		4 => simple.prop_map(|name| Op { name, args: vec![] }),
		1 => Just(Op { name: "toggleHighlight", args: vec![] }),
		1 => colour.clone().prop_map(with("setHighlight", "color")),
		1 => colour.clone().prop_map(with("toggleHighlight", "color")),
		1 => colour.clone().prop_map(with("setColor", "color")),
		1 => colour.prop_map(with("setBackgroundColor", "backgroundColor")),
		1 => family.prop_map(with("setFontFamily", "fontFamily")),
		1 => size.prop_map(with("setFontSize", "fontSize")),
		1 => Just(Op { name: "setLineHeight", args: vec![("lineHeight", Value::from("1.5"))] }),
	]
}

/// Plain ASCII words, so byte and character offsets agree. HTML collapses
/// leading, trailing, and repeated spaces, so words are single-spaced.
fn text() -> impl Strategy<Value = String> {
	"[a-z]{1,6}( [a-z]{1,6}){0,3}"
}

/// Select bytes `from..to` (either order) of the first paragraph.
fn select(editor: &mut Editor, from: u32, to: u32) {
	let block = editor.state().doc().children(None)[0];
	let selection = Selection::Text {
		anchor: Point::new(block, from),
		head: Point::new(block, to),
	};
	editor.select(selection, &mut env()).expect("selects");
}

fn apply(editor: &mut Editor, op: &Op) {
	let args: Attrs = op.args.iter().cloned().collect();
	editor
		.run_named(op.name, &args, &mut env())
		.unwrap_or_else(|error| panic!("{} runs: {error}", op.name));
}

/// The whole document copied as a slice.
fn whole(editor: &Editor) -> canvist_core::Slice {
	let mut copy = editor_from(Some(&editor.state().doc().to_json()));
	let block = copy.state().doc().children(None)[0];
	let len = copy
		.state()
		.doc()
		.inline(block)
		.map_or(0, canvist_model::InlineText::len);
	select(&mut copy, 0, len);
	copy.copy().expect("copy")
}

/// Every character's marks: no character is both subscript and superscript.
fn scripts_exclude_each_other(editor: &Editor) -> bool {
	let schema = editor.schema();
	let (Some(sub), Some(sup)) = (
		schema.mark_type("subscript"),
		schema.mark_type("superscript"),
	) else {
		return false;
	};

	editor
		.state()
		.doc()
		.text_blocks()
		.filter_map(canvist_model::Block::inline)
		.all(|text| {
			text.runs()
				.all(|run| !(run.marks.get(sub).is_some() && run.marks.get(sup).is_some()))
		})
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(96))]

	#[test]
	fn formatting_keeps_invariants_round_trips_and_undoes(
		text in text(),
		steps in prop::collection::vec((op(), any::<prop::sample::Index>(), any::<prop::sample::Index>()), 1..12),
	) {
		let mut editor = typed(&text);
		let initial = editor.state().doc().to_json();
		let len = u32::try_from(text.len()).expect("short text");

		for (op, from, to) in &steps {
			let from = u32::try_from(from.index(text.len() + 1)).expect("short");
			let to = u32::try_from(to.index(text.len() + 1)).expect("short");
			select(&mut editor, from.min(len), to.min(len));
			apply(&mut editor, op);
			assert_valid(editor.state());
			prop_assert!(scripts_exclude_each_other(&editor), "after {op:?}");
		}

		// ProseMirror JSON reads back to the same document.
		let json = editor.state().doc().to_json();
		let reloaded = editor_from(Some(&json));
		prop_assert_eq!(reloaded.state().doc().to_json(), json.clone());

		// HTML reads back to the same HTML; Markdown to the same Markdown.
		let schema = editor.schema().clone();
		let slice = whole(&editor);
		let html = clipboard::to_html(&schema, &slice);
		prop_assert_eq!(clipboard::to_html(&schema, &clipboard::from_html(&schema, &html)), html.clone());
		let markdown = clipboard::to_markdown(&schema, &slice);
		prop_assert_eq!(
			clipboard::to_markdown(&schema, &clipboard::from_markdown(&schema, &markdown)),
			markdown.clone()
		);
		prop_assert_eq!(
			clipboard::to_plain_text(&schema, &clipboard::from_markdown(&schema, &markdown)),
			text.clone(),
			"Markdown keeps the text"
		);

		// Undoing everything restores the plain text exactly.
		let mut undone = 0;
		while editor.undo(&mut env()).expect("undo applies").is_some() {
			undone += 1;
			prop_assert!(undone < 64, "undo terminates");
		}
		let paragraph = &editor.state().doc().to_json()["content"][0];
		let typed = &initial["content"][0];
		prop_assert_eq!(&paragraph["content"], &typed["content"], "undo all ({} steps)", undone);
	}

	#[test]
	fn setting_then_clearing_a_style_restores_the_document(
		text in text(),
		from in any::<prop::sample::Index>(),
		to in any::<prop::sample::Index>(),
		which in 0usize..5,
	) {
		let (set, unset, key) = [
			("setColor", "unsetColor", "color"),
			("setBackgroundColor", "unsetBackgroundColor", "backgroundColor"),
			("setFontFamily", "unsetFontFamily", "fontFamily"),
			("setFontSize", "unsetFontSize", "fontSize"),
			("setLineHeight", "unsetLineHeight", "lineHeight"),
		][which];
		let mut editor = typed(&text);
		let before = editor.state().doc().to_json();
		let (from, to) = (from.index(text.len() + 1), to.index(text.len() + 1));
		select(&mut editor, u32::try_from(from).expect("short"), u32::try_from(to).expect("short"));

		apply(&mut editor, &Op { name: set, args: vec![(key, Value::from("1"))] });
		let shared = editor.query(key, &Attrs::new()).expect("queries answer");
		prop_assert_eq!(shared, Value::from("1"), "the selection shares the value");
		apply(&mut editor, &Op { name: unset, args: vec![] });

		prop_assert_eq!(editor.state().doc().to_json(), before);
	}

	#[test]
	fn toggling_a_mark_twice_restores_the_document(
		text in text(),
		from in any::<prop::sample::Index>(),
		to in any::<prop::sample::Index>(),
		toggle in prop::sample::select(vec![
			("toggleSubscript", None),
			("toggleSuperscript", None),
			("toggleHighlight", None),
			("toggleHighlight", Some("#fef08a")),
		]),
	) {
		let (from, to) = (from.index(text.len() + 1), to.index(text.len() + 1));
		prop_assume!(from != to, "a caret toggles stored marks, not text");
		let mut editor = typed(&text);
		let before = editor.state().doc().to_json();
		select(&mut editor, u32::try_from(from).expect("short"), u32::try_from(to).expect("short"));
		let (name, colour) = toggle;
		let op = Op {
			name,
			args: colour.map(|colour| ("color", Value::from(colour))).into_iter().collect(),
		};

		apply(&mut editor, &op);
		prop_assert_ne!(editor.state().doc().to_json(), before.clone());
		apply(&mut editor, &op);
		prop_assert_eq!(editor.state().doc().to_json(), before);
	}
}
