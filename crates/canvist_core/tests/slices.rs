//! Copying selections into slices and pasting slices back.

mod support;

use canvist_core::EditKind;
use canvist_core::Slice;
use canvist_core::commands;
use canvist_model::BlockTree;
use canvist_model::TreeContent;
use support::dedent;
use support::render;
use support::state;

fn names(state: &canvist_core::EditorState, slice: &Slice) -> Vec<String> {
	let mut names = Vec::new();

	for tree in slice.blocks() {
		tree.walk(&mut |node: &BlockTree| {
			let name = &state.schema().block(node.kind).name;
			let text = match &node.content {
				TreeContent::Inline(text) => format!("{name}: {}", text.as_str()),
				TreeContent::Blocks(_) | TreeContent::Leaf => name.to_string(),
			};

			names.push(text);
		});
	}

	names
}

fn paste(before: &str, slice_from: &str) -> String {
	let source = state(slice_from);
	let slice = commands::slice_selection(&source).expect("copy");
	let mut target = state(before);
	commands::replace_selection(&mut target, slice).expect("paste");
	render(&target)
}

#[test]
fn copying_inside_one_block_keeps_marks() {
	let state = state("heading: a^b{bold:cd}|e");
	let slice = commands::slice_selection(&state).expect("copy");

	assert_eq!(names(&state, &slice), ["heading: bcd"]);
	assert_eq!(slice.to_plain_text(), "bcd");
}

#[test]
fn copying_list_items_keeps_the_list() {
	let state = state(
		"
		bulletList
		  listItem
		    paragraph: o^ne
		  listItem
		    paragraph: tw|o
		  listItem
		    paragraph: three
		",
	);
	let slice = commands::slice_selection(&state).expect("copy");

	assert_eq!(
		names(&state, &slice),
		[
			"bulletList",
			"listItem",
			"paragraph: ne",
			"listItem",
			"paragraph: tw",
		]
	);
}

#[test]
fn copying_blocks_and_empty_selections() {
	let blocks = state(
		"
		paragraph: a
		horizontalRule @
		",
	);
	let caret = state("paragraph: a|");

	assert_eq!(
		names(&blocks, &commands::slice_selection(&blocks).expect("copy")),
		["horizontalRule"]
	);
	assert!(commands::slice_selection(&caret).expect("copy").is_empty());
}

#[test]
fn pasting_inline_text_merges_into_the_block() {
	let result = paste("paragraph: x|y", "paragraph: a^{bold:bc}|d");

	assert_eq!(result, "paragraph: x{bold:bc}|y");
}

#[test]
fn pasting_part_of_a_heading_into_an_empty_paragraph_adopts_its_type() {
	let result = paste("paragraph: |", "heading level=2: ^Title|");

	assert_eq!(result, "heading level=2: Title|");
}

#[test]
fn pasting_several_blocks_opens_both_ends() {
	let result = paste(
		"paragraph: x|y",
		"
		paragraph: ^one
		heading: mid
		paragraph: two|
		",
	);

	assert_eq!(
		result,
		dedent(
			"
		paragraph: xone
		heading: mid
		paragraph: two|y
	"
		)
	);
}

#[test]
fn pasting_a_list_into_an_empty_paragraph_replaces_it() {
	let result = paste(
		"paragraph: |",
		"
		bulletList
		  listItem
		    paragraph: ^one
		  listItem
		    paragraph: two|
		",
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: two|
	"
		)
	);
}

#[test]
fn pasting_into_a_list_item_flattens_what_it_cannot_hold() {
	let result = paste(
		"
		bulletList
		  listItem
		    paragraph: a|b
		",
		"
		paragraph: ^x
		blockquote
		  heading: quoted
		paragraph: y|
		",
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: ax
		    paragraph: quoted
		    paragraph: y|b
	"
		)
	);
}

#[test]
fn pasting_over_a_block_selection_replaces_it() {
	let mut target = state(
		"
		paragraph: a
		horizontalRule @
		",
	);
	let slice = Slice::from_plain_text(target.schema(), "one\ntwo");
	let update = commands::replace_selection(&mut target, slice)
		.expect("paste")
		.expect("applied");

	assert_eq!(update.kind, EditKind::Paste);
	assert_eq!(
		render(&target),
		dedent(
			"
		paragraph: aone
		paragraph: two|
	"
		)
	);
}

#[test]
fn pasted_blocks_get_fresh_ids() {
	let mut state = state("paragraph: one|");
	let slice =
		commands::slice_selection(&support::state("paragraph: ^a\nparagraph: b|")).expect("copy");
	commands::replace_selection(&mut state, slice.clone()).expect("paste");
	commands::replace_selection(&mut state, slice).expect("paste again");

	assert_eq!(state.doc().plain_text(), "onea\nba\nb");
}

#[test]
fn plain_text_round_trips_through_slices() {
	let state = state("paragraph: |");
	let slice = Slice::from_plain_text(state.schema(), "a\r\nb\rc");

	assert_eq!(slice.blocks().len(), 3);
	assert_eq!(slice.to_plain_text(), "a\nb\nc");
	assert_eq!(
		commands::replace_selection(&mut state.clone(), Slice::default()),
		Ok(None)
	);
}
