//! Typing, deleting inside blocks, marks, and IME composition.

mod support;

use canvist_editor::EditKind;
use canvist_editor::ParagraphNavigator;
use canvist_editor::Unit;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::Embed;
use canvist_model::Mark;
use support::after;
use support::dedent;
use support::mark;
use support::render;
use support::state;

#[test]
fn typing_inserts_at_the_caret() {
	let result = after("paragraph: he|o", |state| {
		commands::insert_text(state, "ll")
	});

	assert_eq!(result, "paragraph: hell|o");
}

#[test]
fn typing_nothing_does_nothing() {
	let mut state = state("paragraph: a|");

	assert_eq!(commands::insert_text(&mut state, ""), Ok(None));
}

#[test]
fn typing_replaces_a_range_across_blocks() {
	let result = after(
		"
		paragraph: on^e
		paragraph: tw|o
		",
		|state| commands::insert_text(state, "X"),
	);

	assert_eq!(result, "paragraph: onX|o");
}

#[test]
fn typing_replaces_selected_blocks_with_a_paragraph() {
	let result = after(
		"
		paragraph: a
		horizontalRule @
		paragraph: b
		",
		|state| commands::insert_text(state, "new"),
	);

	assert_eq!(
		result,
		dedent(
			"
		paragraph: a
		paragraph: new|
		paragraph: b
	"
		)
	);
}

#[test]
fn typing_extends_inclusive_marks_but_not_links() {
	let bold = after("paragraph: {bold:bo|}", |state| {
		commands::insert_text(state, "ld")
	});
	let link = after("paragraph: {link href=x:site|}", |state| {
		commands::insert_text(state, "!")
	});
	let inside = after("paragraph: {link href=x:si|te}", |state| {
		commands::insert_text(state, "-")
	});
	let start = after("paragraph: |{bold:x}", |state| {
		commands::insert_text(state, "y")
	});

	assert_eq!(bold, "paragraph: {bold:bold}|");
	assert_eq!(link, "paragraph: {link href=x:site}!|");
	assert_eq!(inside, "paragraph: {link href=x:si-|te}");
	assert_eq!(start, "paragraph: {bold:y|x}");
}

#[test]
fn toggling_a_mark_at_the_caret_affects_the_next_text() {
	let mut state = state("paragraph: a|");
	let bold = mark(&state, "bold");
	let update = commands::toggle_mark(&mut state, &bold)
		.expect("toggle")
		.expect("applied");

	assert_eq!(update.kind, EditKind::Selection);
	assert!(commands::is_mark_active(&state, bold.kind));
	commands::insert_text(&mut state, "b").expect("type");
	commands::insert_text(&mut state, "c").expect("type");
	assert_eq!(render(&state), "paragraph: a{bold:bc}|");

	commands::toggle_mark(&mut state, &bold).expect("toggle off");
	commands::insert_text(&mut state, "d").expect("type");
	assert_eq!(render(&state), "paragraph: a{bold:bc}d|");
}

#[test]
fn stored_marks_clear_when_the_caret_moves() {
	let mut state = state("paragraph: ab|");
	let bold = mark(&state, "bold");
	commands::toggle_mark(&mut state, &bold).expect("toggle");
	canvist_editor::motion::move_caret(
		&mut state,
		canvist_editor::Direction::Backward,
		Unit::Grapheme,
		false,
		&mut ParagraphNavigator,
	)
	.expect("move");

	assert!(state.stored_marks().is_none());
	assert!(!commands::is_mark_active(&state, bold.kind));
}

#[test]
fn toggling_a_mark_over_a_range_adds_then_removes_it() {
	let mut state = state(
		"
		paragraph: o^ne
		paragraph: {bold:tw}|o
		",
	);
	let bold = mark(&state, "bold");

	commands::toggle_mark(&mut state, &bold).expect("add");
	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: o^{bold:ne}
		paragraph: {bold:tw}|o
	"
		)
	);

	let mut full = support::state(
		"
		paragraph: ^{bold:abc}|
		",
	);
	commands::toggle_mark(&mut full, &bold).expect("remove");
	assert_eq!(render(&full), "paragraph: ^abc|");
}

#[test]
fn code_excludes_bold_and_set_mark_replaces_attributes() {
	let mut state = state("paragraph: ^{bold:abc}|");
	let code = mark(&state, "code");
	commands::set_mark(&mut state, &code).expect("code");
	assert_eq!(render(&state), "paragraph: ^{code:abc}|");

	let mut linked = support::state("paragraph: ^{link href=a:abc}|");
	let link = Mark::with_attrs(
		linked.schema().mark_type("link").expect("link"),
		Attrs::new().with("href", "b"),
	);
	commands::set_mark(&mut linked, &link).expect("relink");
	assert_eq!(render(&linked), "paragraph: ^{link href=b:abc}|");

	commands::unset_mark(&mut linked, link.kind).expect("unlink");
	assert_eq!(render(&linked), "paragraph: ^abc|");
}

#[test]
fn mark_commands_do_nothing_on_atomic_selections() {
	let mut state = state("horizontalRule @\nparagraph: x");
	let bold = mark(&state, "bold");

	assert_eq!(commands::toggle_mark(&mut state, &bold), Ok(None));
}

#[test]
fn marks_at_follows_inclusivity_at_boundaries() {
	let state = state("paragraph: {link href=x:a}{bold:b}c");
	let doc = state.doc();
	let block = doc.first_text_block().expect("text");
	let marks = |offset| commands::marks_at(doc, canvist_model::Point::new(block, offset));

	assert!(marks(0).is_empty(), "links do not extend backwards");
	assert!(marks(1).is_empty(), "links do not extend forwards");
	assert_eq!(marks(2).as_slice().len(), 1, "bold extends forwards");
	assert!(marks(3).is_empty());
}

#[test]
fn embeds_carry_marks_and_count_as_one_character() {
	let mut state = state("paragraph: {bold:hi |}");
	let mention = Embed {
		kind: state.schema().embed_type("mention").expect("mention"),
		attrs: Attrs::new().with("id", "ada"),
	};
	commands::insert_embed(&mut state, mention).expect("embed");
	assert_eq!(
		render(&state),
		"paragraph: {bold:hi }{bold:{@mention id=ada}}|"
	);

	commands::delete_backward(&mut state, Unit::Grapheme, &mut ParagraphNavigator).expect("delete");
	assert_eq!(render(&state), "paragraph: {bold:hi }|");
}

#[test]
fn deleting_by_grapheme_removes_whole_clusters() {
	let backward = after("paragraph: a👍🏽|b", |state| {
		commands::delete_backward(state, Unit::Grapheme, &mut ParagraphNavigator)
	});
	let forward = after("paragraph: a|🇬🇧b", |state| {
		commands::delete_forward(state, Unit::Grapheme, &mut ParagraphNavigator)
	});

	assert_eq!(backward, "paragraph: a|b");
	assert_eq!(forward, "paragraph: a|b");
}

#[test]
fn deleting_by_word_skips_punctuation() {
	let backward = after("paragraph: one, two|", |state| {
		commands::delete_backward(state, Unit::Word, &mut ParagraphNavigator)
	});
	let mid = after("paragraph: one, |two", |state| {
		commands::delete_backward(state, Unit::Word, &mut ParagraphNavigator)
	});
	let forward = after("paragraph: |one, two", |state| {
		commands::delete_forward(state, Unit::Word, &mut ParagraphNavigator)
	});
	let spaces = after("paragraph:    |x", |state| {
		commands::delete_backward(state, Unit::Word, &mut ParagraphNavigator)
	});

	assert_eq!(backward, "paragraph: one, |");
	assert_eq!(mid, "paragraph: |two");
	assert_eq!(forward, "paragraph: |, two");
	assert_eq!(spaces, "paragraph: |x");
}

#[test]
fn deleting_to_the_line_boundary_uses_the_navigator() {
	let backward = after("paragraph: one two|three", |state| {
		commands::delete_backward(state, Unit::Line, &mut ParagraphNavigator)
	});
	let forward = after("paragraph: one|two", |state| {
		commands::delete_forward(state, Unit::Line, &mut ParagraphNavigator)
	});

	assert_eq!(backward, "paragraph: |three");
	assert_eq!(forward, "paragraph: one|");
}

#[test]
fn deleting_to_the_document_start_merges_blocks() {
	let result = after(
		"
		paragraph: one
		paragraph: tw|o
		",
		|state| commands::delete_backward(state, Unit::Document, &mut ParagraphNavigator),
	);

	assert_eq!(result, "paragraph: |o");
}

#[test]
fn deleting_a_range_reports_its_kind() {
	let mut inside = state("paragraph: a^bc|d");
	let update = commands::delete_selection(&mut inside)
		.expect("delete")
		.expect("applied");
	let mut caret = state("paragraph: a|");

	assert_eq!(update.kind, EditKind::Deleting);
	assert_eq!(render(&inside), "paragraph: a|d");
	assert_eq!(commands::delete_selection(&mut caret), Ok(None));
}

#[test]
fn composition_replaces_its_preedit_and_commits_once() {
	let mut state = state("paragraph: a|b");
	commands::set_composition(&mut state, "k").expect("preedit");
	commands::set_composition(&mut state, "ka").expect("preedit");
	assert_eq!(render(&state), "paragraph: aka|b");
	let composition = state.composition().expect("composing");
	assert_eq!((composition.start, composition.end), (1, 3));

	commands::commit_composition(&mut state, "か").expect("commit");
	assert_eq!(render(&state), "paragraph: aか|b");
	assert!(state.composition().is_none());
}

#[test]
fn cancelling_a_composition_removes_the_preedit() {
	let mut state = state("paragraph: ^ab|c");
	commands::set_composition(&mut state, "x").expect("preedit");
	assert_eq!(render(&state), "paragraph: x|c");

	commands::cancel_composition(&mut state).expect("cancel");
	assert_eq!(render(&state), "paragraph: |c");
	assert_eq!(commands::cancel_composition(&mut state), Ok(None));
}

#[test]
fn committing_without_a_composition_types_the_text() {
	let result = after("paragraph: a|", |state| {
		commands::commit_composition(state, "é")
	});

	assert_eq!(result, "paragraph: aé|");
}
