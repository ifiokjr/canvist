//! Property tests: random command sequences keep every model invariant and
//! a valid selection, and every update's inverse restores the document
//! exactly while replaying its steps redoes it.

mod support;

use canvist_editor::Direction;
use canvist_editor::EditorState;
use canvist_editor::ParagraphNavigator;
use canvist_editor::Unit;
use canvist_editor::commands;
use canvist_editor::commands::ListType;
use canvist_editor::motion;
use canvist_editor::position;
use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::Content;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use proptest::prelude::*;

const LINES: ParagraphNavigator = ParagraphNavigator;
const DIRECTIONS: [Direction; 2] = [Direction::Backward, Direction::Forward];
const UNITS: [Unit; 4] = [Unit::Grapheme, Unit::Word, Unit::Line, Unit::Document];
const MARKS: [&str; 4] = ["bold", "italic", "code", "link"];
const LISTS: [(&str, &str); 3] = [
	("bulletList", "listItem"),
	("orderedList", "listItem"),
	("taskList", "taskItem"),
];

/// A command described by seeds, resolved against the current state.
#[derive(Clone, Debug)]
enum Op {
	Type(String),
	Backspace(usize),
	Delete(usize),
	Enter,
	SplitItem(usize),
	Sink(usize),
	Lift(usize),
	JoinItem(usize),
	ToggleList(usize),
	ToggleMark(usize),
	Heading(i64),
	Quote,
	LiftBlock,
	Divider,
	Move(usize, usize, bool),
	Vertical(usize, bool),
	SelectAll,
	SelectParent,
	Select(usize, usize, usize, usize),
	CopyPaste(usize),
	Compose(String),
	Commit(String),
}

fn op() -> impl Strategy<Value = Op> {
	let seed = || 0..64usize;
	prop_oneof![
		6 => "[a-z é👍🏽]{1,4}".prop_map(Op::Type),
		3 => (0..4usize).prop_map(Op::Backspace),
		2 => (0..4usize).prop_map(Op::Delete),
		3 => Just(Op::Enter),
		2 => (0..3usize).prop_map(Op::SplitItem),
		2 => (0..3usize).prop_map(Op::Sink),
		2 => (0..3usize).prop_map(Op::Lift),
		2 => (0..3usize).prop_map(Op::JoinItem),
		3 => (0..3usize).prop_map(Op::ToggleList),
		2 => (0..4usize).prop_map(Op::ToggleMark),
		1 => (1..4i64).prop_map(Op::Heading),
		1 => Just(Op::Quote),
		1 => Just(Op::LiftBlock),
		1 => Just(Op::Divider),
		3 => (0..2usize, 0..4usize, any::<bool>()).prop_map(|(d, u, e)| Op::Move(d, u, e)),
		1 => (0..2usize, any::<bool>()).prop_map(|(d, e)| Op::Vertical(d, e)),
		1 => Just(Op::SelectAll),
		1 => Just(Op::SelectParent),
		3 => (seed(), seed(), seed(), seed()).prop_map(|(a, b, c, d)| Op::Select(a, b, c, d)),
		1 => seed().prop_map(Op::CopyPaste),
		1 => "[a-zか]{0,3}".prop_map(Op::Compose),
		1 => "[a-z]{0,2}".prop_map(Op::Commit),
	]
}

fn list(state: &EditorState, index: usize) -> ListType {
	let (list, item) = LISTS[index % LISTS.len()];
	ListType {
		list: support::kind(state, list),
		item: support::kind(state, item),
	}
}

fn known(state: &EditorState) -> Vec<ListType> {
	(0..LISTS.len()).map(|index| list(state, index)).collect()
}

/// A text point picked by seeds.
fn point(doc: &Document, block: usize, offset: usize) -> Option<Point> {
	let texts: Vec<BlockId> = doc.text_blocks().map(Block::id).collect();
	let id = *texts.get(block % texts.len().max(1))?;
	let text = doc.inline(id)?.as_str();
	let boundaries: Vec<usize> = text
		.char_indices()
		.map(|(index, _)| index)
		.chain([text.len()])
		.collect();
	Some(Point::new(id, boundaries[offset % boundaries.len()] as u32))
}

fn run(state: &mut EditorState, op: &Op) -> canvist_editor::CommandResult {
	match op {
		Op::Type(text) => commands::insert_text(state, text),
		Op::Backspace(unit) => commands::delete_backward(state, UNITS[*unit], &LINES),
		Op::Delete(unit) => commands::delete_forward(state, UNITS[*unit], &LINES),
		Op::Enter => commands::split_block(state),
		Op::SplitItem(index) => commands::split_list_item(state, list(state, *index).item),
		Op::Sink(index) => commands::sink_list_item(state, list(state, *index).item),
		Op::Lift(index) => commands::lift_list_item(state, list(state, *index).item),
		Op::JoinItem(index) => commands::join_list_item_backward(state, list(state, *index).item),
		Op::ToggleList(index) => {
			let known = known(state);
			commands::toggle_list(state, list(state, *index), &known)
		}
		Op::ToggleMark(index) => {
			let mark = support::mark(state, MARKS[*index]);
			commands::toggle_mark(state, &mark)
		}
		Op::Heading(level) => {
			let heading = support::kind(state, "heading");
			commands::toggle_block_type(state, heading, &Attrs::new().with("level", *level))
		}
		Op::Quote => {
			let quote = support::kind(state, "blockquote");
			commands::toggle_wrap(state, quote, Attrs::new())
		}
		Op::LiftBlock => commands::lift_block(state),
		Op::Divider => {
			let divider = support::kind(state, "horizontalRule");
			commands::insert_leaf(state, divider, Attrs::new())
		}
		Op::Move(direction, unit, extend) => {
			motion::move_caret(state, DIRECTIONS[*direction], UNITS[*unit], *extend, &LINES)
		}
		Op::Vertical(direction, extend) => {
			motion::move_vertical(state, DIRECTIONS[*direction], *extend, &LINES)
		}
		Op::SelectAll => commands::select_all(state),
		Op::SelectParent => commands::select_parent(state),
		Op::Select(block_a, offset_a, block_b, offset_b) => {
			let doc = state.doc();
			let (Some(anchor), Some(head)) = (
				point(doc, *block_a, *offset_a),
				point(doc, *block_b, *offset_b),
			) else {
				return Ok(None);
			};
			commands::set_selection(state, Selection::text(anchor, head))
		}
		Op::CopyPaste(seed) => {
			let slice = commands::slice_selection(state)?;
			let Some(target) = point(state.doc(), *seed, *seed) else {
				return Ok(None);
			};
			commands::set_selection(state, Selection::caret(target))?;
			commands::replace_selection(state, slice)
		}
		Op::Compose(text) => commands::set_composition(state, text),
		Op::Commit(text) => commands::commit_composition(state, text),
	}
}

/// Panic unless the document's structure, runs, and the state's selection
/// and composition are all valid.
fn assert_valid(state: &EditorState) {
	let doc = state.doc();
	let mut seen = 0;
	check_children(doc, None, &mut seen);
	assert_eq!(seen, doc.block_count(), "every block is reachable once");
	assert!(
		!doc.children(None).is_empty(),
		"the document is never empty"
	);

	for block in doc.iter() {
		if let Content::Inline(text) = block.content() {
			let mut end = 0;

			for run in text.runs() {
				assert_eq!(run.range.start, end, "runs are contiguous");
				assert!(run.range.end > run.range.start, "runs are non-empty");
				end = run.range.end;
			}

			assert_eq!(end, text.len(), "runs cover the text");
		}
	}

	assert_eq!(
		position::valid_selection(doc, state.selection()),
		state.selection(),
		"the selection is valid"
	);

	if let Some(composition) = state.composition() {
		let len = position::text_len(doc, composition.block);
		assert!(composition.start <= composition.end && composition.end <= len);
	}
}

fn check_children(doc: &Document, parent: Option<BlockId>, seen: &mut usize) {
	let parent_kind = parent.and_then(|id| doc.block(id)).map(Block::kind);

	for &child in doc.children(parent) {
		let block = doc.block(child).expect("child ids resolve");
		*seen += 1;
		assert_eq!(block.parent(), parent, "parent pointers match");
		assert!(
			doc.schema().allows_child(parent_kind, block.kind()),
			"the schema allows each child"
		);
		check_children(doc, Some(child), seen);
	}
}

const START: &str = "
	paragraph: intro {bold:bold} {link href=x:link}|
	bulletList
	  listItem
	    paragraph: one
	    bulletList
	      listItem
	        paragraph: nested
	  listItem
	    paragraph: two
	blockquote
	  paragraph: quoted
	horizontalRule
	taskList
	  taskItem
	    paragraph: task
	heading level=2: end
";

proptest! {
	#![proptest_config(ProptestConfig::with_cases(256))]

	#[test]
	fn random_commands_keep_invariants_and_invert_exactly(ops in prop::collection::vec(op(), 1..40)) {
		let mut state = support::state(START);
		assert_valid(&state);

		for op in &ops {
			let before = state.doc().clone();
			let update = run(&mut state, op).expect("commands never produce invalid steps");
			assert_valid(&state);

			let Some(update) = update else {
				prop_assert_eq!(state.doc().to_json(), before.to_json(), "a command that does not apply changes nothing");
				continue;
			};

			let mut undone = state.doc().clone();
			undone.apply_steps(update.commit.undo_steps()).expect("inverses apply");
			prop_assert_eq!(undone.to_json(), before.to_json(), "undo restores {:?}", op);

			undone.apply_steps(update.commit.steps.clone()).expect("steps replay");
			prop_assert_eq!(undone.to_json(), state.doc().to_json(), "redo replays {:?}", op);
		}
	}
}
