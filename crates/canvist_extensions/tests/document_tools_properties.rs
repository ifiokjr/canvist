//! Property tests for the document tools: replacing every match matches
//! a reference model and undoes in one step, random block moves follow a
//! list model, and clearing formatting keeps the text and is idempotent.

#![cfg(feature = "full")]

mod support;

use canvist_core::Command;
use canvist_extensions::DragHandleHandle;
use canvist_model::Point;
use canvist_model::Selection;
use proptest::prelude::*;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;
use support::document_tools::apply;
use support::document_tools::block;
use support::document_tools::can;
use support::document_tools::env;
use support::document_tools::paragraphs;
use support::document_tools::select;
use support::document_tools::texts;
use support::document_tools::tools;

const CASES: u32 = 96;

/// A small alphabet, so terms often match, with case and width variations.
const LETTERS: [&str; 8] = ["a", "b", "A", "é", "É", "日", "😀", " "];

/// The letters a search term draws from: no space, and only one of `é`
/// and `É`, so a case-insensitive term still has an uppercase twin.
const TERM_LETTERS: [&str; 6] = ["a", "b", "A", "é", "日", "😀"];

/// The marks a generated text run may carry.
const MARK_NAMES: [&str; 5] = ["bold", "italic", "code", "strike", "underline"];

fn word() -> impl Strategy<Value = String> {
	prop::collection::vec(prop::sample::select(LETTERS.to_vec()), 0..8)
		.prop_map(|parts| parts.concat())
}

fn term() -> impl Strategy<Value = String> {
	prop::collection::vec(prop::sample::select(TERM_LETTERS.to_vec()), 1..3)
		.prop_map(|parts| parts.concat())
}

/// What a block's text becomes when every match of `term` is replaced,
/// left to right without overlap.
fn replaced(text: &str, term: &str, replacement: &str, exact: bool) -> String {
	if exact {
		return text.replace(term, replacement);
	}

	// Every character in the alphabet lowercases to one character of the
	// same width, so the lowercase text's byte offsets are the original's.
	let lower = text.to_lowercase();
	let wanted = term.to_lowercase();
	let mut out = String::new();
	let mut from = 0;

	for (start, found) in lower.match_indices(wanted.as_str()) {
		out.push_str(&text[from..start]);
		out.push_str(replacement);
		from = start + found.len();
	}

	out.push_str(&text[from..]);
	out
}

/// One drag-handle action on a top-level paragraph.
#[derive(Clone, Debug)]
enum Move {
	Upward,
	Downward,
	/// Drop the block before the block at this index, or at the end.
	Before(usize),
}

impl Move {
	/// The drag handle's command for this action in a list of `len` blocks.
	fn command(&self, drag: DragHandleHandle, len: usize) -> Command {
		match *self {
			Self::Upward => drag.up(),
			Self::Downward => drag.down(),
			Self::Before(to) => drag.move_to(to.min(len)),
		}
	}

	/// Where the block at `index` lands in a list of `len` blocks, or
	/// `None` when the action leaves it where it is.
	fn destination(&self, index: usize, len: usize) -> Option<usize> {
		match *self {
			Self::Upward => index.checked_sub(1),
			Self::Downward => (index + 1 < len).then_some(index + 1),
			Self::Before(to) => {
				let to = to.min(len);
				// Dropping after the block itself closes its own gap first.
				(to != index && to != index + 1).then(|| to - usize::from(to > index))
			}
		}
	}
}

fn moves() -> impl Strategy<Value = Vec<(usize, Move)>> {
	prop::collection::vec(
		(
			0..6_usize,
			prop_oneof![
				Just(Move::Upward),
				Just(Move::Downward),
				(0..8_usize).prop_map(Move::Before),
			],
		),
		1..12,
	)
}

/// A block of the kinds clearing formatting touches, with marked text.
fn rich_block() -> impl Strategy<Value = Json> {
	let text = (
		"[a-z]{1,4}",
		prop::sample::subsequence(MARK_NAMES.to_vec(), 0..3),
	)
		.prop_map(|(text, marks)| {
			let marks: Vec<Json> = marks
				.into_iter()
				.map(|mark| json!({ "type": mark }))
				.collect();
			if marks.is_empty() {
				json!({ "type": "text", "text": text })
			} else {
				json!({ "type": "text", "text": text, "marks": marks })
			}
		});
	let inline = prop::collection::vec(text, 1..3);

	inline.prop_flat_map(|content| {
		let paragraph = json!({ "type": "paragraph", "content": content });
		prop_oneof![
			Just(paragraph.clone()),
			(1..4_i64).prop_map({
				let content = content.clone();
				move |level| json!({ "type": "heading", "attrs": { "level": level }, "content": content })
			}),
			Just(json!({ "type": "blockquote", "content": [paragraph.clone()] })),
			Just(json!({
				"type": "bulletList",
				"content": [{ "type": "listItem", "content": [paragraph.clone()] }]
			})),
			Just(json!({
				"type": "taskList",
				"content": [{ "type": "taskItem", "attrs": { "checked": true }, "content": [paragraph] }]
			})),
		]
	})
}

/// The text of every text block, in document order.
fn text_blocks(typist: &Typist) -> Vec<String> {
	typist
		.editor
		.state()
		.doc()
		.text_blocks()
		.map(|block| {
			block
				.inline()
				.map(canvist_model::InlineText::as_str)
				.unwrap_or_default()
				.to_owned()
		})
		.collect()
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(CASES))]

	#[test]
	fn replace_all_replaces_exactly_the_matches_and_undoes_in_one_step(
		blocks in prop::collection::vec(word(), 1..4),
		term in term(),
		replacement in prop::sample::select(vec!["", "x", "😀😀", "aA"]),
		exact in any::<bool>(),
	) {
		let texts: Vec<&str> = blocks.iter().map(String::as_str).collect();
		let (mut typist, tools) = tools(Some(&paragraphs(&texts)));
		let before = typist.snapshot();
		let expected: Vec<String> = blocks
			.iter()
			.map(|text| replaced(text, &term, replacement, exact))
			.collect();

		prop_assert!(apply(&mut typist, &tools.find.set_term(&term)));
		prop_assert!(apply(&mut typist, &tools.find.set_case_sensitive(exact)));
		let found = tools.find.matches(typist.editor.state()).len();
		let applied = apply(&mut typist, &tools.find.replace_all(replacement));

		prop_assert_eq!(applied, found > 0);
		prop_assert_eq!(text_blocks(&typist), expected);
		support::assert_valid(typist.editor.state());

		if applied {
			typist.editor.undo(&mut env()).expect("undo applies");
			prop_assert_eq!(typist.snapshot(), before.clone());
			prop_assert!(!typist.editor.can_undo(), "replacing all was one step");
			typist.editor.redo(&mut env()).expect("redo applies");
			prop_assert_eq!(
				text_blocks(&typist),
				blocks
					.iter()
					.map(|text| replaced(text, &term, replacement, exact))
					.collect::<Vec<_>>()
			);
		}
	}

	/// Random moves keep the blocks in the order a plain list model
	/// predicts, the caret travels with its block, and undoing every move
	/// restores the document exactly.
	#[test]
	fn block_moves_follow_a_list_model_and_undo_exactly(script in moves()) {
		let names = ["a", "b", "c", "d", "e", "f"];
		let (mut typist, tools) = tools(Some(&paragraphs(&names)));
		let before = typist.snapshot();
		let mut model: Vec<&str> = names.to_vec();
		let mut steps = 0;

		for (position, action) in script {
			let target = block(&typist, position);
			select(&mut typist, Selection::caret(Point::new(target, 0)));
			let index = model
				.iter()
				.position(|&name| name == texts(&typist)[position])
				.expect("the model has every block");
			let command = action.command(tools.drag, model.len());
			let destination = action.destination(index, model.len());

			prop_assert_eq!(apply(&mut typist, &command), destination.is_some());

			if let Some(destination) = destination {
				let moved = model.remove(index);
				model.insert(destination, moved);
				steps += 1;
			}

			prop_assert_eq!(texts(&typist), model.iter().map(|&name| name.to_owned()).collect::<Vec<_>>());
			prop_assert_eq!(
				typist.editor.state().selection(),
				Selection::caret(Point::new(target, 0)),
				"the caret moves with its block"
			);
		}

		for _ in 0..steps {
			typist.editor.undo(&mut env()).expect("undo applies");
		}

		prop_assert_eq!(texts(&typist), names.map(str::to_owned).to_vec());
		prop_assert_eq!(
			typist.snapshot()["doc"].clone(),
			before["doc"].clone(),
			"undo restores the order exactly"
		);
	}

	#[test]
	fn clear_formatting_keeps_the_text_and_is_idempotent(
		blocks in prop::collection::vec(rich_block(), 1..5),
	) {
		let doc = json!({ "type": "doc", "content": blocks });
		let (mut typist, tools) = tools(Some(&doc));
		let text = text_blocks(&typist);
		typist.type_script("[Mod-a]");

		apply(&mut typist, &tools.clear.clear());
		let cleared = typist.snapshot();
		support::assert_valid(typist.editor.state());
		// The trailing node may add an empty paragraph; every generated
		// block has text.
		let mut after = text_blocks(&typist);
		after.retain(|text| !text.is_empty());
		prop_assert_eq!(after, text, "the text survives in order");

		for child in cleared["doc"].as_array().into_iter().flatten() {
			prop_assert_eq!(&child["type"], "paragraph");
			prop_assert!(child.get("attrs").is_none());

			for run in child["content"].as_array().into_iter().flatten() {
				prop_assert!(run.get("marks").is_none(), "no marks left: {run}");
			}
		}

		typist.type_script("[Mod-a]");
		prop_assert!(!can(&mut typist, &tools.clear.clear_nodes()));
		prop_assert!(!can(&mut typist, &tools.clear.unset_all_marks()));
		prop_assert!(!apply(&mut typist, &tools.clear.clear()));
		prop_assert_eq!(typist.snapshot()["doc"].clone(), cleared["doc"].clone());
	}
}
