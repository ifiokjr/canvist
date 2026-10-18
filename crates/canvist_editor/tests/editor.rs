//! The editor: keymaps, history, input and paste rules, and extensions.

mod support;

use std::time::Duration;

use canvist_editor::BaseKeymap;
use canvist_editor::Binding;
use canvist_editor::CommandResult;
use canvist_editor::EditKind;
use canvist_editor::Editor;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Env;
use canvist_editor::Extension;
use canvist_editor::HistoryConfig;
use canvist_editor::InputRule;
use canvist_editor::KeyPress;
use canvist_editor::Modifiers;
use canvist_editor::PasteRule;
use canvist_editor::Platform;
use canvist_editor::Slice;
use canvist_editor::UndoHistory;
use canvist_editor::Update;
use canvist_editor::block_type;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::mark_type;
use canvist_editor::undo;
use canvist_model::Attrs;
use canvist_model::BlockTypeId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use support::dedent;
use support::render;

/// The test schema plus a handful of rules and bindings.
struct TestKit;

impl Extension for TestKit {
	fn name(&self) -> &'static str {
		"testKit"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		support::specs(schema)
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-z", std::sync::Arc::new(undo))])
	}

	fn commands(
		&self,
		schema: &Schema,
	) -> Result<Vec<(&'static str, canvist_editor::Command)>, EditorError> {
		let bold = mark_type(schema, self.name(), "bold")?;
		let toggle =
			command(move |state| commands::toggle_mark(state, &canvist_model::Mark::new(bold)));
		Ok(vec![("toggleBold", toggle)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let heading = block_type(schema, self.name(), "heading")?;
		let divider = block_type(schema, self.name(), "horizontalRule")?;
		let bold = mark_type(schema, self.name(), "bold")?;
		let level = |found: &canvist_editor::RuleMatch| {
			let hashes = found.group(1).map_or(1, str::len);
			Attrs::new().with("level", hashes as i64)
		};

		Ok(vec![
			InputRule::block_type(r"^(#{1,6})\s$", heading, level)?,
			InputRule::mark(r"(?:^|\s)\*\*([^*\s](?:[^*]*[^*\s])?)\*\*$", bold)?,
			InputRule::text(r"->$", "→")?,
			InputRule::leaf(r"^---$", divider)?,
		])
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		let bold = mark_type(schema, self.name(), "bold")?;
		let link = mark_type(schema, self.name(), "link")?;
		Ok(vec![
			PasteRule::mark(r"\*\*([^*]+)\*\*", bold)?,
			PasteRule::mark_with(r"https?://\S+", link, |url| Attrs::new().with("href", url))?,
		])
	}

	fn code_blocks(&self, schema: &Schema) -> Result<Vec<BlockTypeId>, EditorError> {
		Ok(vec![block_type(schema, self.name(), "codeBlock")?])
	}
}

/// Keeps an empty paragraph at the end of the document.
struct Trailing;

impl Extension for Trailing {
	fn name(&self) -> &'static str {
		"trailing"
	}

	fn after_update(&self, state: &mut EditorState, _update: &Update) -> CommandResult {
		let doc = state.doc();
		let last = doc.children(None).last().copied();
		let paragraph = doc.schema().default_text_block();
		let ends_in_text = last
			.and_then(|id| doc.block(id))
			.is_some_and(|block| block.kind() == paragraph);

		if ends_in_text {
			return Ok(None);
		}

		let selection = state.selection();
		let index = doc.children(None).len();
		state.change(EditKind::Structure, |tr| {
			let id = tr.allocate_id()?;
			let block =
				canvist_model::BlockTree::text(id, paragraph, canvist_model::InlineText::new());
			tr.insert_blocks(None, index, vec![block])?;
			tr.set_selection(selection);
			Ok(true)
		})
	}
}

fn editor(outline: &str) -> Editor {
	editor_on(outline, Platform::Other)
}

fn editor_on(outline: &str, platform: Platform) -> Editor {
	let mut selection = None;
	let mut editor = Editor::builder()
		.extension(TestKit)
		.extension(BaseKeymap)
		.platform(platform)
		.build_with(|schema, _client| {
			let (doc, marked) = support::parse(&schema, outline);
			selection = marked;
			Ok(doc)
		})
		.expect("editor builds");

	if let Some(selection) = selection {
		editor.select(selection, &at(0)).expect("select");
	}

	editor
}

fn at(millis: u64) -> Env<'static> {
	Env::headless(Duration::from_millis(millis))
}

fn key(name: &str) -> KeyPress<'_> {
	KeyPress::plain(name)
}

fn with(name: &str, modifiers: Modifiers) -> KeyPress<'_> {
	KeyPress::new(name, modifiers)
}

const CTRL: Modifiers = Modifiers {
	ctrl: true,
	..Modifiers::NONE
};
const META: Modifiers = Modifiers {
	meta: true,
	..Modifiers::NONE
};

fn type_text(editor: &mut Editor, text: &str, start: u64) {
	for (index, character) in text.chars().enumerate() {
		let mut buffer = [0; 4];
		editor
			.insert_text(
				character.encode_utf8(&mut buffer),
				&at(start + index as u64 * 10),
			)
			.expect("type");
	}
}

#[test]
fn base_keymap_edits_and_moves() {
	let mut editor = editor("paragraph: ab|c");
	editor.handle_key(key("Enter"), &at(0)).expect("enter");
	editor.handle_key(key("ArrowLeft"), &at(0)).expect("left");
	editor.handle_key(key("Delete"), &at(0)).expect("join");

	assert_eq!(render(editor.state()), "paragraph: ab|c");
	assert_eq!(editor.handle_key(key("F13"), &at(0)), Ok(None));
}

#[test]
fn mod_means_cmd_on_apple_and_ctrl_elsewhere() {
	let mut apple = editor_on("paragraph: a|b", Platform::Apple);
	let mut other = editor_on("paragraph: a|b", Platform::Other);

	assert_eq!(
		apple
			.handle_key(with("a", CTRL), &at(0))
			.map(|update| update.is_some()),
		Ok(true)
	);
	assert_eq!(
		render(apple.state()),
		"paragraph: |ab",
		"Ctrl-a is line start on Apple"
	);
	apple
		.handle_key(with("a", META), &at(0))
		.expect("select all");
	other
		.handle_key(with("a", CTRL), &at(0))
		.expect("select all");

	assert_eq!(render(apple.state()), "paragraph: ^ab|");
	assert_eq!(render(other.state()), "paragraph: ^ab|");
	assert_eq!(apple.platform(), Platform::Apple);
}

#[test]
fn word_deletion_uses_the_platform_modifier() {
	let mut apple = editor_on("paragraph: one two|", Platform::Apple);
	let mut other = editor_on("paragraph: one two|", Platform::Other);
	let alt = Modifiers {
		alt: true,
		..Modifiers::NONE
	};
	apple
		.handle_key(with("Backspace", alt), &at(0))
		.expect("delete word");
	other
		.handle_key(with("Backspace", CTRL), &at(0))
		.expect("delete word");

	assert_eq!(render(apple.state()), "paragraph: one |");
	assert_eq!(render(other.state()), "paragraph: one |");
}

#[test]
fn typing_quickly_undoes_as_one_step() {
	let mut editor = editor("paragraph: |");
	type_text(&mut editor, "hello", 0);
	type_text(&mut editor, " world", 2_000);

	editor.undo(&at(3_000)).expect("undo");
	assert_eq!(render(editor.state()), "paragraph: hello|");
	editor.undo(&at(3_000)).expect("undo");
	assert_eq!(render(editor.state()), "paragraph: |");
	assert!(!editor.can_undo());

	editor.redo(&at(3_000)).expect("redo");
	editor.redo(&at(3_000)).expect("redo");
	assert_eq!(render(editor.state()), "paragraph: hello world|");
	assert!(!editor.can_redo());
}

#[test]
fn deleting_and_typing_are_separate_steps() {
	let mut editor = editor("paragraph: abc|");
	editor.handle_key(key("Backspace"), &at(0)).expect("delete");
	type_text(&mut editor, "x", 10);

	editor.undo(&at(20)).expect("undo typing");
	assert_eq!(render(editor.state()), "paragraph: ab|");
	editor.undo(&at(20)).expect("undo deleting");
	assert_eq!(render(editor.state()), "paragraph: abc|");
}

#[test]
fn undo_restores_the_selection_from_before_the_change() {
	let mut editor = editor("paragraph: a^bc|d");
	type_text(&mut editor, "X", 0);
	editor.undo(&at(10)).expect("undo");

	assert_eq!(render(editor.state()), "paragraph: a^bc|d");
}

#[test]
fn moving_the_caret_breaks_a_typing_group() {
	let mut editor = editor("paragraph: |");
	type_text(&mut editor, "ab", 0);
	editor.handle_key(key("ArrowLeft"), &at(30)).expect("left");
	type_text(&mut editor, "c", 40);
	editor.undo(&at(50)).expect("undo");

	assert_eq!(render(editor.state()), "paragraph: a|b");
}

#[test]
fn new_edits_clear_redo_and_history_has_a_depth() {
	let mut editor = Editor::builder()
		.extension(TestKit)
		.history(UndoHistory::new(HistoryConfig {
			group_delay: Duration::ZERO,
			depth: 2,
		}))
		.build()
		.expect("editor");
	type_text(&mut editor, "abc", 0);
	editor.undo(&at(100)).expect("undo");
	assert!(editor.can_redo());
	type_text(&mut editor, "d", 200);

	assert!(!editor.can_redo());
	editor.undo(&at(300)).expect("undo d");
	editor.undo(&at(300)).expect("undo b");
	assert_eq!(
		editor.undo(&at(300)),
		Ok(None),
		"depth 2 dropped the oldest step"
	);
	assert_eq!(editor.state().doc().plain_text(), "a");
}

#[test]
fn remote_changes_clear_local_history() {
	let mut editor = editor("paragraph: |");
	type_text(&mut editor, "ab", 0);
	let block = editor.state().doc().first_text_block().expect("text");
	let remote = canvist_model::Step::InsertInline {
		block,
		offset: 0,
		content: canvist_model::InlineText::plain("Z", canvist_model::MarkSet::empty()),
	};
	let update = editor.apply_remote([remote], &at(100)).expect("remote");

	assert_eq!(update.kind, EditKind::Remote);
	assert_eq!(render(editor.state()), "paragraph: Zab|");
	assert!(!editor.can_undo());
}

#[test]
fn heading_rule_fires_and_backspace_restores_the_text() {
	let mut editor = editor("paragraph: |");
	type_text(&mut editor, "## ", 0);
	assert_eq!(render(editor.state()), "heading level=2: |");

	editor
		.handle_key(key("Backspace"), &at(100))
		.expect("undo rule");
	assert_eq!(render(editor.state()), "paragraph: ## |");

	editor
		.handle_key(key("Backspace"), &at(200))
		.expect("delete");
	assert_eq!(render(editor.state()), "paragraph: ##|");
}

#[test]
fn undoing_a_reverted_rule_brings_it_back() {
	let mut editor = editor("paragraph: |");
	type_text(&mut editor, "# ", 0);
	editor
		.handle_key(key("Backspace"), &at(100))
		.expect("undo rule");
	editor.handle_key(with("z", CTRL), &at(200)).expect("undo");

	assert_eq!(render(editor.state()), "heading: |");
}

#[test]
fn rules_do_not_fire_after_the_caret_moves() {
	let mut editor = editor("paragraph: |");
	type_text(&mut editor, "# ", 0);
	editor
		.handle_key(key("ArrowRight"), &at(100))
		.expect("no-op move");
	type_text(&mut editor, "x", 200);
	editor.handle_key(key("ArrowLeft"), &at(300)).expect("move");
	editor
		.handle_key(key("Backspace"), &at(400))
		.expect("nothing to join");

	assert_eq!(render(editor.state()), "heading: |x");
}

#[test]
fn mark_rules_format_and_stop_extending() {
	let mut editor = editor("paragraph: |");
	type_text(&mut editor, "a **bold** c", 0);

	assert_eq!(render(editor.state()), "paragraph: a {bold:bold} c|");
}

#[test]
fn text_and_leaf_rules() {
	let mut arrow = editor("paragraph: |");
	type_text(&mut arrow, "a->b", 0);
	let mut divider = editor("paragraph: |");
	type_text(&mut divider, "---", 0);

	assert_eq!(render(arrow.state()), "paragraph: a→b|");
	assert_eq!(
		render(divider.state()),
		dedent(
			"
		horizontalRule
		paragraph: |
	"
		)
	);
}

#[test]
fn rules_skip_code_blocks_and_compositions() {
	let mut code = editor("codeBlock: |");
	type_text(&mut code, "## ", 0);
	let mut composing = editor("paragraph: ##|");
	composing.set_composition("x", &at(0)).expect("preedit");
	composing.insert_text(" ", &at(10)).expect("type");

	assert_eq!(render(code.state()), "codeBlock: ## |");
	assert_eq!(render(composing.state()), "paragraph: ##x |");
}

#[test]
fn rule_patterns_must_be_anchored_and_valid() {
	let kind = support::schema().block_type("heading").expect("heading");
	let unanchored = InputRule::block_type("^# ", kind, |_| Attrs::new());
	let invalid = PasteRule::mark("(", support::schema().mark_type("bold").expect("bold"));

	assert!(matches!(
		unanchored,
		Err(EditorError::InvalidPattern { .. })
	));
	assert!(matches!(invalid, Err(EditorError::InvalidPattern { .. })));
}

#[test]
fn paste_rules_format_pasted_text() {
	let mut editor = editor("paragraph: |");
	editor
		.paste_text("see **this** at https://canvist.dev now", &at(0))
		.expect("paste");

	assert_eq!(
		render(editor.state()),
		"paragraph: see {bold:this} at {link href=https://canvist.dev:https://canvist.dev} now|"
	);
}

#[test]
fn pasting_into_code_keeps_text_verbatim() {
	let mut editor = editor("codeBlock: |");
	let update = editor
		.paste_text("**a**\nb", &at(0))
		.expect("paste")
		.expect("applied");

	assert_eq!(update.kind, EditKind::Paste);
	assert_eq!(editor.state().doc().plain_text(), "**a**\nb");
}

#[test]
fn copy_cut_and_paste_through_the_editor() {
	let mut editor = editor("paragraph: a^bc|d");
	let copied = editor.copy().expect("copy");
	let (cut, update) = editor.cut(&at(0)).expect("cut");

	assert_eq!(copied, cut);
	assert!(update.is_some());
	editor
		.paste(Slice::default(), &at(10))
		.expect("empty paste");
	editor.paste(cut, &at(20)).expect("paste");
	assert_eq!(render(editor.state()), "paragraph: abc|d");
}

#[test]
fn compositions_flow_through_history_as_one_step() {
	let mut editor = editor("paragraph: |");
	editor.set_composition("k", &at(0)).expect("preedit");
	editor.set_composition("ka", &at(10)).expect("preedit");
	editor.commit_composition("か", &at(20)).expect("commit");
	assert_eq!(render(editor.state()), "paragraph: か|");

	editor.undo(&at(30)).expect("undo");
	assert_eq!(render(editor.state()), "paragraph: |");
	editor.set_composition("x", &at(40)).expect("preedit");
	editor.cancel_composition(&at(50)).expect("cancel");
	assert_eq!(render(editor.state()), "paragraph: |");
}

#[test]
fn named_commands_run_and_unknown_names_do_nothing() {
	let mut editor = editor("paragraph: ^ab|");
	editor.run_named("toggleBold", &at(0)).expect("bold");

	assert_eq!(render(editor.state()), "paragraph: ^{bold:ab}|");
	assert!(editor.command("toggleBold").is_some());
	assert_eq!(editor.run_named("nope", &at(0)), Ok(None));
}

#[test]
fn after_update_fixes_are_folded_into_the_same_step() {
	let mut editor = Editor::builder()
		.extension(TestKit)
		.extension(Trailing)
		.build()
		.expect("editor");
	let divider = editor
		.schema()
		.block_type("horizontalRule")
		.expect("divider");
	let insert = command(move |state| commands::insert_leaf(state, divider, Attrs::new()));
	editor.run(&insert, &at(0)).expect("insert");
	let update = editor
		.run(&command(commands::select_all), &at(10))
		.expect("select");

	assert!(update.is_some());
	assert_eq!(
		render(editor.state()),
		dedent(
			"
		horizontalRule @
		paragraph: @
	"
		)
	);
	editor.undo(&at(20)).expect("undo");
	assert_eq!(render(editor.state()), "paragraph: |");
}

#[test]
fn missing_types_and_bad_bindings_fail_the_build() {
	struct Needy;
	impl Extension for Needy {
		fn name(&self) -> &'static str {
			"needy"
		}

		fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
			block_type(schema, self.name(), "table")?;
			Ok(Vec::new())
		}
	}

	struct BadKey;
	impl Extension for BadKey {
		fn name(&self) -> &'static str {
			"badKey"
		}

		fn keymap(
			&self,
			_schema: &Schema,
			_platform: Platform,
		) -> Result<Vec<Binding>, EditorError> {
			Ok(vec![("Hyper-x", command(commands::select_all))])
		}
	}

	let needy = Editor::builder()
		.extension(TestKit)
		.extension(Needy)
		.build();
	let bad_key = Editor::builder()
		.extension(TestKit)
		.extension(BadKey)
		.build();

	assert!(matches!(
		needy,
		Err(EditorError::MissingType { name: "table", .. })
	));
	assert_eq!(
		bad_key.err(),
		Some(EditorError::InvalidKey("Hyper-x".to_owned()))
	);
}
