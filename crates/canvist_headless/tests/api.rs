//! The headless API beyond what the conformance fixtures cover: thread
//! safety, typed handles, custom extension sets, and snapshots.

#![cfg(feature = "starter-kit")]

use canvist_extensions::Bold;
use canvist_extensions::Document;
use canvist_extensions::Paragraph;
use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::ErrorCode;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use canvist_headless::Snapshot;
use serde_json::Value as Json;
use serde_json::json;

const fn assert_send_sync<T: Send + Sync>() {}
const fn assert_send<T: Send>() {}

#[test]
fn engines_are_shared_across_threads_and_docs_move_between_them() {
	const _: () = assert_send_sync::<Engine<canvist_extensions::StarterKitHandles>>();
	const _: () = assert_send::<Doc>();

	let engine = Engine::starter_kit();
	let workers: Vec<_> = (0..4)
		.map(|index| {
			let engine = engine.clone();
			std::thread::spawn(move || {
				let doc = engine.parse(Format::Markdown, &format!("# Doc {index}"))?;
				Ok::<_, HeadlessError>(doc.export(Format::Text))
			})
		})
		.collect();
	let texts: Vec<String> = workers
		.into_iter()
		.map(|worker| worker.join().expect("no panic").expect("parses"))
		.collect();

	assert_eq!(texts, ["Doc 0", "Doc 1", "Doc 2", "Doc 3"]);
}

#[test]
fn an_engine_without_an_extension_has_neither_its_handle_nor_its_names() -> Result<(), HeadlessError>
{
	let engine = Engine::new(|builder| {
		builder.add(Document);
		builder.add(Paragraph);
		builder.add(Bold)
	})?;
	let bold = engine.handles();
	let mut doc = engine.parse(Format::Markdown, "# Not a heading here")?;

	// Markdown falls back to paragraphs for blocks the schema lacks.
	assert_eq!(doc.export(Format::Html), "<p>Not a heading here</p>");

	doc.select_all()?;
	assert!(doc.run(&bold.toggle())?);
	assert!(bold.is_active(doc.state()));

	let error = doc
		.run_named("toggleItalic", &Json::Null)
		.expect_err("italic is not registered");
	assert_eq!(error.code(), ErrorCode::UnknownName);

	let mut names: Vec<&str> = engine
		.metadata()
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(|extension| extension["name"].as_str())
		.collect();
	names.sort_unstable();
	assert_eq!(names, ["bold", "doc", "paragraph"]);
	Ok(())
}

#[test]
fn chains_apply_together_or_not_at_all() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let kit = engine.handles();
	let mut doc = engine.parse(Format::Markdown, "Hello")?;
	doc.select_all()?;

	assert!(doc.chain(&[kit.bold.toggle(), kit.italic.toggle()])?);
	assert_eq!(doc.export(Format::Markdown), "***Hello***");

	// Lifting a list item does not apply outside a list, so nothing changes.
	assert!(!doc.chain(&[kit.strike.toggle(), kit.list_item.lift()])?);
	assert_eq!(doc.export(Format::Markdown), "***Hello***");

	assert!(doc.undo()?);
	assert_eq!(
		doc.export(Format::Markdown),
		"Hello",
		"the chain was one undo step"
	);
	Ok(())
}

#[test]
fn snapshots_keep_block_ids_and_reject_other_shapes() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut doc = engine.parse(Format::Markdown, "one\n\ntwo")?;
	doc.select_range(&Position::new([1], 1), &Position::new([1], 2))?;

	let snapshot = Snapshot::from_json(&doc.snapshot().to_json())?;
	let restored = engine.restore(&snapshot)?;

	assert_eq!(restored.to_json(), doc.to_json(), "ids survive");
	assert_eq!(restored.selected_range(), doc.selected_range());

	for invalid in [
		json!({}),
		json!({ "version": 2, "content": {} }),
		json!({ "version": 1, "content": {}, "selection": { "anchor": 1 } }),
	] {
		let error = Snapshot::from_json(&invalid).expect_err("not a snapshot");
		assert_eq!(error.code(), ErrorCode::Snapshot, "{invalid}");
	}

	Ok(())
}

#[test]
fn formats_parse_from_their_names() {
	for format in Format::ALL {
		assert_eq!(format.name().parse::<Format>().ok(), Some(format));
	}

	let error = "docx".parse::<Format>().expect_err("not a format");
	assert_eq!(error.code(), ErrorCode::Parse);
}

#[test]
fn queries_and_arguments_cross_as_json() -> Result<(), HeadlessError> {
	let mut doc = Engine::starter_kit().parse(Format::Markdown, "Title")?;

	assert_eq!(doc.query("headingLevel", &Json::Null)?, Json::Null);
	assert!(doc.run_named("setHeading", &json!({ "level": 4 }))?);
	assert_eq!(doc.query("headingLevel", &json!({}))?, json!(4));

	for args in [json!([1]), json!({ "level": { "deep": 1 } })] {
		let error = doc
			.run_named("setHeading", &args)
			.expect_err("not valid arguments");
		assert_eq!(error.code(), ErrorCode::InvalidArgs, "{args}");
	}

	Ok(())
}

#[test]
fn the_readme_example_holds() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let kit = engine.handles();
	let mut doc = engine.parse(Format::Markdown, "Hello world")?;

	doc.select_range(&Position::new([0], 6), &Position::new([0], 11))?;
	doc.run(&kit.bold.toggle())?;

	assert_eq!(
		doc.export(Format::Html),
		"<p>Hello <strong>world</strong></p>"
	);
	Ok(())
}
