//! The typed API: handles, named commands and queries, metadata, dry runs,
//! and chains, across the whole StarterKit.

mod support;

use std::time::Duration;

use canvist_editor::Command;
use canvist_editor::Editor;
use canvist_editor::EditorError;
use canvist_editor::Env;
use canvist_editor::NodeKind;
use canvist_extensions::HeadingLevel;
use canvist_extensions::StarterKit;
use canvist_extensions::StarterKitHandles;
use canvist_model::Attrs;
use canvist_model::Value;
use insta::assert_json_snapshot;
use support::Typist;

/// A StarterKit editor built through the typed path, with its handles.
fn typed() -> (Editor, StarterKitHandles) {
	let mut builder = Editor::builder();
	let kit = StarterKit::add_all(&mut builder);
	(builder.build().expect("the StarterKit builds"), kit)
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

/// A typist over a typed StarterKit editor holding `script`.
fn typed_with(script: &str) -> (Typist, StarterKitHandles) {
	let (editor, kit) = typed();
	let mut typist = Typist::with(editor);
	typist.type_script(script);
	(typist, kit)
}

#[test]
fn metadata_matches_the_schema() {
	let (editor, _) = typed();
	let schema = editor.schema();

	for extension in editor.metadata() {
		for node in &extension.nodes {
			let defaults = match node.kind {
				NodeKind::Block => {
					let kind = schema.block_type(node.name).expect("declared blocks exist");
					schema.block(kind).attrs.clone()
				}
				NodeKind::Mark => {
					let kind = schema.mark_type(node.name).expect("declared marks exist");
					schema.mark(kind).attrs.clone()
				}
				NodeKind::Embed => {
					let kind = schema.embed_type(node.name).expect("declared embeds exist");
					schema.embed(kind).attrs.clone()
				}
			};

			let declared: Vec<&str> = node.attrs.iter().map(|attr| attr.name).collect();
			let actual: Vec<&str> = defaults.iter().map(|(key, _)| key).collect();
			assert_eq!(
				actual.len(),
				declared.len(),
				"{} declares every attribute",
				node.name
			);

			for attr in &node.attrs {
				let default = defaults.get(attr.name).expect("declared attributes exist");
				assert_eq!(
					attr.default.as_ref(),
					Some(default),
					"{}.{}",
					node.name,
					attr.name
				);
				assert!(
					attr.ty.accepts(default),
					"{}.{} default has its type",
					node.name,
					attr.name
				);
			}
		}
	}
}

#[test]
fn every_command_runs_with_sample_arguments_and_rejects_missing_ones() {
	let (mut typist, _) =
		typed_with("# Title⏎- one⏎two⏎⏎[[ ] task⏎⏎> quote⏎⏎text[Shift-ArrowLeft][Shift-ArrowLeft]");
	let metadata = typist.editor.metadata().to_vec();

	for command in metadata.iter().flat_map(|extension| &extension.commands) {
		let args: Attrs = command
			.args
			.iter()
			.map(|arg| (arg.name, arg.ty.sample()))
			.collect();
		let built = typist.editor.command_named(command.name, &args);
		let ran = built.and_then(|command| typist.editor.run(&command, &env()));
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
	let (typist, _) = typed_with("## Hi⏎[[x] done⏎⏎1. one");
	let metadata = typist.editor.metadata();

	for query in metadata.iter().flat_map(|extension| &extension.queries) {
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
fn same_as_named(script: &str, typed: fn(&StarterKitHandles) -> Command, name: &str, args: &Attrs) {
	let (mut by_handle, kit) = typed_with(script);
	let (mut by_name, _) = typed_with(script);
	by_handle
		.editor
		.run(&typed(&kit), &env())
		.expect("typed command");
	by_name
		.editor
		.run_named(name, args, &env())
		.expect("named command");

	assert_eq!(by_handle.snapshot(), by_name.snapshot(), "{name}");
}

#[test]
fn typed_handles_and_named_commands_agree() {
	let none = Attrs::new();
	same_as_named("hello[Mod-a]", |kit| kit.bold.toggle(), "toggleBold", &none);
	same_as_named("hello[Mod-a]", |kit| kit.italic.set(), "setItalic", &none);
	same_as_named(
		"hello",
		|kit| kit.heading.toggle(HeadingLevel::H3),
		"toggleHeading",
		&Attrs::new().with("level", 3),
	);
	same_as_named(
		"hello",
		|kit| kit.code_block.set(Some("rust")),
		"setCodeBlock",
		&Attrs::new().with("language", "rust"),
	);
	same_as_named(
		"hello",
		|kit| kit.bullet_list.toggle(),
		"toggleBulletList",
		&none,
	);
	same_as_named(
		"1. a",
		|kit| kit.ordered_list.set_start(7),
		"setOrderedListStart",
		&Attrs::new().with("start", 7),
	);
	same_as_named(
		"[[ ] a",
		|kit| kit.task_list.toggle_checked(),
		"toggleTask",
		&none,
	);

	same_as_named(
		"hi[Mod-a]",
		|kit| kit.link.set("https://canvist.dev"),
		"setLink",
		&Attrs::new().with("href", "https://canvist.dev"),
	);
	same_as_named(
		"a",
		|kit| kit.horizontal_rule.insert(),
		"setHorizontalRule",
		&none,
	);
	same_as_named("a", |kit| kit.hard_break.insert(), "setHardBreak", &none);
	same_as_named(
		"> a",
		|kit| kit.blockquote.toggle(),
		"toggleBlockquote",
		&none,
	);
	same_as_named("- a⏎b", |kit| kit.list_item.sink(), "sinkListItem", &none);
}

#[test]
fn typed_queries_read_the_selection() {
	let (heading, kit) = typed_with("### Title");
	assert_eq!(
		kit.heading.level(heading.editor.state()),
		Some(HeadingLevel::H3)
	);
	assert_eq!(
		heading.editor.query("headingLevel", &Attrs::new()),
		Ok(Value::Int(3))
	);

	let (code, kit) = typed_with("```rust x");
	assert_eq!(
		kit.code_block.language(code.editor.state()).as_deref(),
		Some("rust")
	);
	assert!(kit.code_block.is_active(code.editor.state()));

	let (task, kit) = typed_with("[[x] done");
	assert_eq!(kit.task_list.is_checked(task.editor.state()), Some(true));
	assert!(kit.task_list.is_active(task.editor.state()));

	let (ordered, kit) = typed_with("4. four");
	assert_eq!(kit.ordered_list.start(ordered.editor.state()), Some(4));

	let (link, kit) = typed_with("see https://canvist.dev ←←");
	assert_eq!(
		kit.link.href(link.editor.state()).as_deref(),
		Some("https://canvist.dev")
	);
	assert!(!kit.bold.is_active(link.editor.state()));
	assert!(kit.paragraph.is_active(link.editor.state()));
}

#[test]
fn can_and_chains_work_through_handles() {
	let (mut typist, kit) = typed_with("text[Mod-a]");
	let before = typist.snapshot();

	assert_eq!(typist.editor.can(&kit.bold.toggle(), &env()), Ok(true));
	assert_eq!(typist.editor.can(&kit.history.redo(), &env()), Ok(false));
	assert_eq!(typist.snapshot(), before);

	typist
		.editor
		.chain()
		.then(kit.heading.set(HeadingLevel::H1))
		.then(kit.bold.toggle())
		.run(&env())
		.expect("chain");
	assert_eq!(
		kit.heading.level(typist.editor.state()),
		Some(HeadingLevel::H1)
	);
	assert!(kit.bold.is_active(typist.editor.state()));

	typist
		.editor
		.run(&kit.history.undo(), &env())
		.expect("undo");
	assert_eq!(typist.snapshot(), before, "the chain undoes as one step");
}

#[test]
fn starter_kit_metadata() {
	let (editor, _) = typed();
	assert_json_snapshot!(editor.metadata_json());
}
