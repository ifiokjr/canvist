//! Scripted typing through the full StarterKit, snapshotted as JSON.

mod support;

use canvist_editor::Platform;
use insta::assert_json_snapshot;
use support::Typist;
use support::typed;

#[test]
fn headings_and_nested_lists() {
	assert_json_snapshot!(typed("## Hi⏎- one⏎two⇥"));
}

#[test]
fn outdenting_and_leaving_a_list() {
	assert_json_snapshot!(typed("- one⏎two⇥⏎⏎⏎after"));
}

#[test]
fn ordered_lists_start_at_the_typed_number() {
	assert_json_snapshot!(typed("3. third⏎fourth"));
}

#[test]
fn task_lists_from_brackets() {
	assert_json_snapshot!(typed("[[x] done⏎todo"));
}

#[test]
fn blockquotes_wrap_and_enter_on_an_empty_line_leaves() {
	assert_json_snapshot!(typed("> quoted⏎⏎after"));
}

#[test]
fn code_blocks_keep_newlines_and_exit_on_a_third_enter() {
	assert_json_snapshot!(typed("```rust fn⏎main⏎⏎⏎after **not code**"));
}

#[test]
fn rules_do_not_fire_inside_code() {
	assert_json_snapshot!(typed("``` ## not a heading"));
}

#[test]
fn horizontal_rules_from_dashes() {
	assert_json_snapshot!(typed("---below"));
}

#[test]
fn markdown_marks_while_typing() {
	assert_json_snapshot!(typed("**bold** *italic* ~~gone~~ `code` __also__ _too_"));
}

#[test]
fn marks_from_keys() {
	assert_json_snapshot!(typed(
		"plain [Mod-b]bold[Mod-i] both[Mod-b] italic[Mod-i] plain"
	));
}

#[test]
fn links_stop_at_their_edges() {
	assert_json_snapshot!(typed("see https://canvist.dev, then type"));
}

#[test]
fn backspace_after_a_rule_restores_the_text() {
	assert_json_snapshot!("rule_undo_heading", typed("## ⌫"));
	assert_json_snapshot!("rule_undo_list", typed("- ⌫"));
	assert_json_snapshot!("rule_undo_bold", typed("**b**⌫"));
}

#[test]
fn hard_breaks_stay_inside_the_block() {
	assert_json_snapshot!(typed("one[Shift-Enter]two"));
}

#[test]
fn undo_groups_by_pause_and_redo_replays() {
	let mut typist = Typist::new();
	typist.type_script("hello⏸ world[Mod-z]");
	assert_json_snapshot!("after_undo", typist.snapshot());

	typist.type_script("[Mod-Shift-z]");
	assert_json_snapshot!("after_redo", typist.snapshot());
}

#[test]
fn undo_restores_structure_and_selection() {
	assert_json_snapshot!(typed("- a⏎b⇥[Mod-z][Mod-z]"));
}

#[test]
fn backspace_joins_list_items_and_lifts_the_first() {
	assert_json_snapshot!(typed("- one⏎two←←←⌫⌫⌫⌫⌫"));
}

#[test]
fn select_all_and_delete_empties_the_document() {
	assert_json_snapshot!(typed("# Title⏎- item⏎⏎text[Mod-a]⌫"));
}

#[test]
fn toggling_lists_and_headings_with_keys() {
	assert_json_snapshot!(typed(
		"one[Mod-Shift-8][Mod-Shift-7][Mod-Shift-9][Mod-Alt-2]"
	));
}

#[test]
fn apple_word_motion_and_deletion() {
	let mut typist = Typist::on(Platform::Apple);
	typist.type_script("one two three[Alt-ArrowLeft][Alt-Backspace][Mod-ArrowRight]!");
	assert_json_snapshot!(typist.snapshot());
}

#[test]
fn vertical_motion_keeps_its_column() {
	assert_json_snapshot!(typed("abcdef⏎x⏎123456↑↑X"));
}
