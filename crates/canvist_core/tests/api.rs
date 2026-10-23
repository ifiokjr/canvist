//! The headless API: typed handles, dry runs, chains, and metadata.

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_core::BaseKeymap;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::Editor;
use canvist_core::EditorError;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::Mark;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;
use support::render;

/// The test schema with a typed bold API.
#[derive(Clone, Copy, Debug)]
struct Kit;

#[derive(Clone, Copy, Debug)]
struct KitHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods need proof of registration"
)]
impl KitHandle {
	fn toggle_bold(self) -> Command {
		command(|state| {
			let bold = mark_type(state.schema(), "kit", "bold")?;
			commands::toggle_mark(state, &Mark::new(bold))
		})
	}

	fn is_bold(self, editor: &Editor) -> bool {
		editor
			.schema()
			.mark_type("bold")
			.is_some_and(|bold| commands::is_mark_active(editor.state(), bold))
	}
}

impl Extension for Kit {
	fn name(&self) -> &'static str {
		"kit"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		support::specs(schema)
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let handle = KitHandle {
			_registration: registration(),
		};
		Ok(vec![CommandSpec::simple(
			"toggleBold",
			"Toggle bold.",
			handle.toggle_bold(),
		)])
	}

	fn queries(&self, schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let bold = mark_type(schema, self.name(), "bold")?;
		Ok(vec![QuerySpec::new(
			"isBold",
			"Whether bold is active.",
			ValueType::Bool,
			move |state, _| Ok(Value::Bool(commands::is_mark_active(state, bold))),
		)])
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![
			NodeMeta::new(NodeKind::Block, "heading").attr(Field::optional(
				"level",
				ValueType::IntRange { min: 1, max: 6 },
				1,
				"The level.",
			)),
			NodeMeta::new(NodeKind::Mark, "bold"),
		]
	}
}

/// A registration for building the kit's own specs, obtained the only way
/// there is: by adding an extension to a builder.
fn registration() -> Registration {
	struct Probe;
	impl Extension for Probe {
		fn name(&self) -> &'static str {
			"probe"
		}
	}
	impl TypedExtension for Probe {
		type Handle = Registration;

		fn handle(&self, registration: Registration) -> Registration {
			registration
		}
	}
	Editor::builder().add(Probe)
}

impl TypedExtension for Kit {
	type Handle = KitHandle;

	fn handle(&self, registration: Registration) -> KitHandle {
		KitHandle {
			_registration: registration,
		}
	}
}

fn editor_with(outline: &str) -> (Editor, KitHandle) {
	let mut builder = Editor::builder();
	let kit = builder.add(Kit);
	builder.add(BaseKeymap);
	let mut selection = None;
	let mut editor = builder
		.build_with(|schema, _| {
			let (doc, marked) = support::parse(&schema, outline);
			selection = marked;
			Ok(doc)
		})
		.expect("editor");

	if let Some(selection) = selection {
		editor.select(selection, &mut env()).expect("select");
	}

	(editor, kit)
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

/// Content, version, every revision, and the selection.
fn fingerprint(editor: &Editor) -> (String, u64, Vec<(BlockId, u32)>) {
	let doc = editor.state().doc();
	let mut revs: Vec<(BlockId, u32)> = doc.iter().map(|block| (block.id(), block.rev())).collect();
	revs.sort_unstable();
	(render(editor.state()), doc.version(), revs)
}

#[test]
fn typed_handles_run_commands_and_answer_queries() {
	let (mut editor, kit) = editor_with("paragraph: ^ab|");
	editor.run(&kit.toggle_bold(), &mut env()).expect("bold");

	assert!(kit.is_bold(&editor));
	assert_eq!(render(editor.state()), "paragraph: ^{bold:ab}|");
}

#[test]
fn can_answers_without_changing_anything() {
	let (mut editor, kit) = editor_with("paragraph: ^ab|");
	let before = fingerprint(&editor);
	let undo: Command = Arc::new(canvist_core::undo);

	assert_eq!(editor.can(&kit.toggle_bold(), &mut env()), Ok(true));
	assert_eq!(editor.can(&undo, &mut env()), Ok(false));
	assert_eq!(fingerprint(&editor), before, "a dry run leaves no trace");
	assert!(!editor.can_undo());

	editor.run(&kit.toggle_bold(), &mut env()).expect("bold");
	assert_eq!(editor.can(&undo, &mut env()), Ok(true));
	assert!(editor.can_undo(), "a dry undo leaves the history alone");
}

#[test]
fn can_reports_commands_that_do_not_apply() {
	let (mut editor, _) = editor_with("horizontalRule @");
	let split = command(commands::split_block);

	assert_eq!(editor.can(&split, &mut env()), Ok(false));
}

#[test]
fn chains_apply_as_one_undo_step() {
	let (mut editor, kit) = editor_with("paragraph: ^ab|");
	let heading = editor.schema().block_type("heading").expect("heading");
	let level = Attrs::new().with("level", 2);
	let set_heading = command(move |state| commands::set_block_type(state, heading, &level));

	let update = editor
		.chain()
		.then(kit.toggle_bold())
		.then(set_heading)
		.run(&mut env())
		.expect("chain")
		.expect("applied");

	assert_eq!(update.commit.steps.len(), 2);
	assert_eq!(render(editor.state()), "heading level=2: ^{bold:ab}|");
	editor.undo(&mut env()).expect("undo");
	assert_eq!(render(editor.state()), "paragraph: ^ab|");
}

#[test]
fn chains_are_all_or_nothing() {
	let (mut editor, kit) = editor_with("paragraph: ^ab|");
	let before = fingerprint(&editor);
	let split_leaf = command(|state| {
		let divider = state
			.schema()
			.block_type("horizontalRule")
			.expect("divider");
		commands::set_block_type(state, divider, &Attrs::new())
	});

	let chained = editor
		.chain()
		.then(kit.toggle_bold())
		.then(split_leaf.clone());
	assert_eq!(chained.run(&mut env()), Ok(None));
	assert_eq!(
		fingerprint(&editor),
		before,
		"a failed chain rolls back exactly"
	);
	assert_eq!(
		editor.chain().then(kit.toggle_bold()).can(&mut env()),
		Ok(true)
	);
	assert_eq!(editor.chain().then(split_leaf).can(&mut env()), Ok(false));
	assert_eq!(fingerprint(&editor), before);
}

#[test]
fn undo_cannot_be_chained() {
	let (mut editor, kit) = editor_with("paragraph: ^ab|");
	editor.run(&kit.toggle_bold(), &mut env()).expect("bold");
	let undo: Command = Arc::new(canvist_core::undo);

	assert_eq!(editor.chain().then(undo).run(&mut env()), Ok(None));
	assert!(kit.is_bold(&editor));
}

#[test]
fn metadata_describes_commands_queries_and_nodes() {
	let (editor, _) = editor_with("paragraph: |");
	let json = editor.metadata_json();
	let kit = &json[0];

	assert_eq!(kit["name"], "kit");
	assert_eq!(kit["commands"][0]["name"], "toggleBold");
	assert_eq!(kit["queries"][0]["returns"]["type"], "bool");
	assert_eq!(kit["nodes"][0]["attrs"][0]["type"]["max"], 6);
	assert_eq!(editor.metadata().len(), 2);
}

#[test]
fn duplicate_command_names_fail_the_build() {
	let mut builder = Editor::builder();
	builder.add(Kit);

	struct Rival;
	impl Extension for Rival {
		fn name(&self) -> &'static str {
			"rival"
		}

		fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
			Ok(vec![CommandSpec::simple(
				"toggleBold",
				"Also bold.",
				command(commands::select_all),
			)])
		}
	}

	let built = builder.extension(Rival).build();
	assert_eq!(
		built.err(),
		Some(EditorError::DuplicateName("toggleBold".to_owned()))
	);
}

#[test]
fn handles_on_an_editor_without_the_extension_fail_at_run_time() {
	let mut builder = Editor::builder();
	let kit = builder.add(Kit);
	drop(builder);
	let mut other = Editor::builder().extension(Bare).build().expect("editor");

	struct Bare;
	impl Extension for Bare {
		fn name(&self) -> &'static str {
			"bare"
		}

		fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
			schema.block(canvist_model::BlockSpec::text("paragraph"))
		}
	}

	let missing = other.run(&kit.toggle_bold(), &mut env());
	assert!(matches!(
		missing,
		Err(EditorError::MissingType { name: "bold", .. })
	));
	let _ = other.state().doc().iter().map(Block::id).count();
}
