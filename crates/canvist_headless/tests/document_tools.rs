//! The document tools through the headless API: two Yjs peers editing
//! comments, block order, and text concurrently converge, documents keep
//! their comments through every format and through the shared state, and
//! the server-rendered HTML marks commented text without relying on colour.

#![cfg(all(feature = "full", feature = "collab"))]

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::ErrorCode;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use serde_json::Value as Json;
use serde_json::json;

/// The StarterKit plus the document tools, by name.
fn engine() -> Engine {
	Engine::named(&[
		"heading",
		"blockquote",
		"codeBlock",
		"hardBreak",
		"horizontalRule",
		"bulletList",
		"orderedList",
		"listItem",
		"taskList",
		"bold",
		"italic",
		"underline",
		"strike",
		"code",
		"link",
		"history",
		"trailingNode",
		"findAndReplace",
		"tableOfContents",
		"comments",
		"dragHandle",
		"clearFormatting",
	])
	.expect("the document tools register by name")
}

/// Two peers sharing `markdown`: the first as client 1, the second joined
/// from its state as client 2.
fn peers(markdown: &str) -> Result<(Doc, Doc), HeadlessError> {
	let engine = engine();
	let mut first = engine.parse(Format::Markdown, markdown)?;
	first.share(1)?;
	let second = engine.from_yjs(&first.encode_state()?, 2)?;
	first.take_updates()?;

	Ok((first, second))
}

/// Deliver each peer's pending updates to the other.
fn exchange(first: &mut Doc, second: &mut Doc) -> Result<(), HeadlessError> {
	let from_first = first.take_updates()?;
	let from_second = second.take_updates()?;

	for update in &from_first {
		second.apply_update(update)?;
	}

	for update in &from_second {
		first.apply_update(update)?;
	}

	Ok(())
}

/// Both peers hold the same valid document.
fn assert_converged(first: &Doc, second: &Doc) -> Result<(), HeadlessError> {
	assert_eq!(first.to_json(), second.to_json(), "the peers converge");
	assert!(first.validate()?.is_valid());
	assert!(second.validate()?.is_valid());
	Ok(())
}

fn select(doc: &mut Doc, block: u32, from: u32, to: u32) -> Result<(), HeadlessError> {
	doc.select_range(&Position::new([block], from), &Position::new([block], to))
}

fn caret(doc: &mut Doc, path: &[u32], offset: u32) -> Result<(), HeadlessError> {
	let at = Position::new(path.to_vec(), offset);
	doc.select_range(&at, &at)
}

fn comments(doc: &Doc) -> Result<Json, HeadlessError> {
	doc.query("comments", &Json::Null)
}

/// The text of each top-level block, in order.
fn blocks(doc: &Doc) -> Vec<String> {
	doc.export(Format::Text)
		.split('\n')
		.filter(|line| !line.is_empty())
		.map(str::to_owned)
		.collect()
}

#[test]
fn concurrent_comments_on_separate_text_both_survive() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers("Shared words here")?;

	select(&mut first, 0, 0, 6)?;
	assert!(first.run_named("setComment", &json!({ "threadId": "a" }))?);
	select(&mut second, 0, 13, 17)?;
	assert!(second.run_named("setComment", &json!({ "threadId": "b" }))?);
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	assert_eq!(
		comments(&first)?,
		json!([
			{ "threadId": "a", "text": "Shared" },
			{ "threadId": "b", "text": "here" },
		])
	);

	assert!(second.run_named("unsetComment", &json!({ "threadId": "a" }))?);
	exchange(&mut first, &mut second)?;
	assert_converged(&first, &second)?;
	assert_eq!(
		comments(&first)?,
		json!([{ "threadId": "b", "text": "here" }])
	);
	Ok(())
}

/// Overlapping comments from two peers converge, and both threads stay in
/// the document. Where the ranges overlap, the two `threads` lists are
/// concurrent values of one Yjs formatting attribute, so they do not merge,
/// and as with any Yjs format (bold too), the first peer's end marker can
/// cut the second peer's range short.
#[test]
fn concurrent_overlapping_comments_converge() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers("Shared words here")?;

	select(&mut first, 0, 0, 12)?;
	first.run_named("setComment", &json!({ "threadId": "a" }))?;
	select(&mut second, 0, 7, 17)?;
	second.run_named("setComment", &json!({ "threadId": "b" }))?;
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	caret(&mut first, &[0], 3)?;
	assert_eq!(first.query("activeComments", &Json::Null)?, json!(["a"]));
	let threads: Vec<Json> = comments(&first)?
		.as_array()
		.into_iter()
		.flatten()
		.map(|thread| thread["threadId"].clone())
		.collect();
	assert_eq!(threads, [json!("a"), json!("b")]);
	Ok(())
}

#[test]
fn comments_survive_the_shared_state() -> Result<(), HeadlessError> {
	let (mut first, _) = peers("Note this")?;
	select(&mut first, 0, 5, 9)?;
	first.run_named("setComment", &json!({ "threadId": "t 1" }))?;
	first.run_named("setComment", &json!({ "threadId": "t2" }))?;

	let joined = engine().from_yjs(&first.encode_state()?, 3)?;
	assert_eq!(joined.to_json(), first.to_json());
	assert_eq!(
		comments(&joined)?,
		json!([
			{ "threadId": "t 1", "text": "this" },
			{ "threadId": "t2", "text": "this" },
		]),
		"JSON and Yjs keep ids HTML cannot, such as ones with spaces"
	);
	Ok(())
}

#[test]
fn concurrent_block_moves_keep_every_block_once() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers("a\n\nb\n\nc\n\nd")?;

	caret(&mut first, &[0], 0)?;
	assert!(first.run_named("moveBlockDown", &Json::Null)?);
	caret(&mut second, &[3], 0)?;
	assert!(second.run_named("moveBlockUp", &Json::Null)?);
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	assert_eq!(blocks(&first), ["b", "a", "d", "c"]);

	// Both peers move the same block, to different places.
	caret(&mut first, &[1], 0)?;
	assert!(first.run_named("moveBlocksTo", &json!({ "index": 4 }))?);
	caret(&mut second, &[1], 0)?;
	assert!(second.run_named("moveBlocksTo", &json!({ "index": 0 }))?);
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	let mut order = blocks(&first);
	order.sort();
	assert_eq!(order, ["a", "b", "c", "d"], "no block is lost or doubled");

	first.undo()?;
	exchange(&mut first, &mut second)?;
	assert_converged(&first, &second)?;
	let mut order = blocks(&second);
	order.sort();
	assert_eq!(order, ["a", "b", "c", "d"]);
	Ok(())
}

#[test]
fn concurrent_replace_all_and_typing_converge() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers("fish and fish\n\nmore fish")?;

	first.run_named("setSearchTerm", &json!({ "term": "fish" }))?;
	assert!(first.run_named("replaceAll", &json!({ "replacement": "cat" }))?);
	caret(&mut second, &[1], 0)?;
	second.insert_text("so ")?;
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	assert_eq!(first.export(Format::Markdown), "cat and cat\n\nso more cat");
	assert_eq!(
		second.query("searchResults", &Json::Null)?,
		Json::Null,
		"the search term is per peer, not shared"
	);

	first.undo()?;
	exchange(&mut first, &mut second)?;
	assert_converged(&first, &second)?;
	assert_eq!(
		second.export(Format::Markdown),
		"fish and fish\n\nso more fish"
	);
	Ok(())
}

#[test]
fn clearing_formatting_while_a_peer_types_converges() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers("## **Bold** title")?;

	first.select_all()?;
	assert!(first.run_named("clearFormatting", &Json::Null)?);
	caret(&mut second, &[0], 4)?;
	second.insert_text("er")?;
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	assert_eq!(first.export(Format::Markdown), "Bolder title");
	Ok(())
}

/// Moving or lifting a block copies it to its new place in the shared
/// state, since Yjs arrays cannot move items. The peers still converge, but
/// text a peer typed into the block's old copy meanwhile is lost; this test
/// pins convergence, not that loss.
#[test]
fn moving_a_block_a_peer_is_typing_in_converges() -> Result<(), HeadlessError> {
	for (markdown, path, command) in [
		("a\n\nBold title", vec![1], "moveBlockUp"),
		("- a\n- Bold title", vec![0, 1, 0], "moveBlockUp"),
		("> Bold title", vec![0, 0], "clearNodes"),
	] {
		let (mut first, mut second) = peers(markdown)?;
		caret(&mut first, &path, 0)?;
		assert!(first.run_named(command, &Json::Null)?);
		caret(&mut second, &path, 4)?;
		second.insert_text("er")?;
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		assert!(first.export(Format::Text).contains("Bold"), "{markdown}");
	}

	Ok(())
}

#[test]
fn comments_round_trip_through_every_format() -> Result<(), HeadlessError> {
	let engine = engine();
	let tiptap = json!({
		"type": "doc",
		"content": [{
			"type": "paragraph",
			"content": [
				{ "type": "text", "text": "See " },
				{
					"type": "text",
					"text": "here",
					"marks": [{ "type": "comment", "attrs": { "threads": ["x", "y"] } }]
				}
			]
		}]
	});
	let doc = engine.from_json(&tiptap)?;
	let html = doc.export(Format::Html);

	assert_eq!(
		html,
		"<p>See <span data-type=\"comment\" data-threads=\"x y\">here</span></p>"
	);
	assert_eq!(
		engine.parse(Format::Html, &html)?.to_json(),
		doc.to_json(),
		"HTML reads back what it wrote"
	);
	assert_eq!(
		engine
			.parse(Format::Json, &doc.export(Format::Json))?
			.to_json(),
		doc.to_json()
	);
	assert_eq!(
		doc.export(Format::Markdown),
		"See here",
		"Markdown has no comments"
	);
	assert_eq!(
		engine
			.parse(Format::Markdown, &doc.export(Format::Markdown))?
			.query("comments", &Json::Null)?,
		json!([])
	);
	Ok(())
}

#[test]
fn document_tool_errors_have_stable_codes() -> Result<(), HeadlessError> {
	let mut doc = engine().parse(Format::Markdown, "# One\n\ntwo")?;
	let code = |result: Result<bool, HeadlessError>| result.map_err(|error| error.code());

	assert_eq!(
		code(doc.run_named("goToHeading", &json!({ "index": -2 }))),
		Err(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.run_named("moveBlocksTo", &json!({ "index": 1.5 }))),
		Err(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.run_named("setComment", &json!({ "thread": "a" }))),
		Err(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.run_named("setSearchTerm", &json!({ "term": null }))),
		Err(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.run_named("findNext", &Json::Null)),
		Err(ErrorCode::UnknownName)
	);
	assert_eq!(
		code(doc.run_named("goToHeading", &json!({ "index": 5 }))),
		Ok(false)
	);
	assert_eq!(doc.export(Format::Markdown), "# One\n\ntwo");
	Ok(())
}

#[cfg(feature = "ssr")]
#[test]
fn rendered_comments_are_underlined_as_well_as_tinted() -> Result<(), HeadlessError> {
	let mut doc = engine().parse(Format::Text, "Read this")?;
	select(&mut doc, 0, 5, 9)?;
	doc.run_named("setComment", &json!({ "threadId": "a" }))?;

	let page = canvist_headless::Renderer::new().html(&doc, 480.0);
	assert!(
		page.html
			.contains("Read <span class=\"cv-m cv-m-comment\">this</span>"),
		"commented text stays text: {}",
		page.html
	);
	let rule = page
		.css
		.split('}')
		.find(|rule| rule.contains(".cv-m-comment{"))
		.expect("a comment rule");
	assert!(
		rule.contains("text-decoration-line:underline") && rule.contains("background-color"),
		"commented text is told apart by more than colour: {rule}"
	);
	Ok(())
}
