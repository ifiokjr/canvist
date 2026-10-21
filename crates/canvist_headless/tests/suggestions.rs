//! Mentions, hashtags, and emoji through the headless API: Yjs peers
//! editing them concurrently converge, server-side rendering gives each
//! atom readable text, and failures carry stable error codes.

#![cfg(feature = "full")]

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::ErrorCode;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use serde_json::Value as Json;
use serde_json::json;

const ATOMS: [&str; 3] = ["mention", "hashtag", "emoji"];

fn engine(client: u32) -> Engine {
	Engine::named(&ATOMS)
		.expect("the atoms register")
		.with_client(client)
}

fn caret(doc: &mut Doc, offset: u32) -> Result<(), HeadlessError> {
	let position = Position::new([0], offset);
	doc.select_range(&position, &position)
}

fn code(error: &HeadlessError) -> ErrorCode {
	error.code()
}

#[test]
fn failures_carry_stable_codes() -> Result<(), HeadlessError> {
	let mut doc = engine(1).create()?;
	doc.type_text("@a")?;
	let before = doc.to_json();

	let invalid =
		|result: Result<bool, HeadlessError>| code(&result.expect_err("invalid arguments fail"));
	assert_eq!(
		invalid(doc.run_named("insertMention", &json!({}))),
		ErrorCode::InvalidArgs
	);
	assert_eq!(
		invalid(doc.run_named("insertMention", &json!({ "id": 7 }))),
		ErrorCode::InvalidArgs
	);
	assert_eq!(
		invalid(doc.run_named("insertHashtag", &json!({ "tag": ["a"] }))),
		ErrorCode::InvalidArgs
	);
	assert_eq!(
		invalid(doc.run_named("setEmoji", &json!({}))),
		ErrorCode::InvalidArgs
	);
	assert_eq!(
		code(
			&doc.query("searchEmoji", &json!({ "query": "a", "limit": "ten" }))
				.expect_err("the limit is an int")
		),
		ErrorCode::InvalidArgs
	);
	assert_eq!(doc.to_json(), before, "failures change nothing");

	assert!(
		!doc.run_named("setEmoji", &json!({ "name": "not_an_emoji" }))?,
		"an unknown shortcode is not an error, it just does nothing"
	);

	let mut plain = Engine::starter_kit().create()?;
	assert_eq!(
		code(
			&plain
				.run_named("insertMention", &json!({ "id": "1" }))
				.expect_err("no mentions")
		),
		ErrorCode::UnknownName
	);
	assert_eq!(
		code(
			&plain
				.query("hashtags", &json!({}))
				.expect_err("no hashtags")
		),
		ErrorCode::UnknownName
	);
	assert_eq!(
		code(&Engine::named(&["emojis"]).expect_err("an unknown extension")),
		ErrorCode::Config
	);
	Ok(())
}

#[test]
fn atoms_survive_snapshots_with_their_ids() -> Result<(), HeadlessError> {
	let engine = engine(1);
	let mut doc = engine.create()?;
	doc.type_text("@a")?;
	doc.run_named("insertMention", &json!({ "id": "u-7", "label": "Ada" }))?;
	doc.type_text(":smile: #tag ")?;

	let snapshot = canvist_headless::Snapshot::from_json(&doc.snapshot().to_json())?;
	let restored = engine.restore(&snapshot)?;
	assert_eq!(restored.to_json(), doc.to_json());
	assert_eq!(
		restored.to_json()["content"][0]["content"][0]["attrs"]["id"],
		"u-7"
	);
	assert_eq!(restored.query("hashtags", &json!({}))?, json!(["tag"]));
	Ok(())
}

// Collaboration.

/// Two peers sharing `text`: the first shares it as client 1, the second
/// joins from its state as client 2.
fn peers(text: &str) -> Result<(Doc, Doc), HeadlessError> {
	let mut first = engine(1).parse(Format::Text, text)?;
	first.share(1)?;
	let second = engine(2).from_yjs(&first.encode_state()?, 2)?;
	Ok((first, second))
}

/// Deliver each peer's pending updates to the other.
fn sync(first: &mut Doc, second: &mut Doc) -> Result<(), HeadlessError> {
	let from_first = first.take_updates()?;
	let from_second = second.take_updates()?;

	for update in from_first {
		second.apply_update(&update)?;
	}

	for update in from_second {
		first.apply_update(&update)?;
	}

	Ok(())
}

/// The document without block ids, which the peers allocate differently.
fn content(doc: &Doc) -> Json {
	doc.to_json()["content"].clone()
}

/// What one peer does to the shared paragraph, at `offset`.
#[derive(Clone, Copy, Debug)]
enum Edit {
	Mention,
	Shortcode,
	Hashtag,
	PickEmoji,
	InsertHashtag,
}

const EDITS: [Edit; 5] = [
	Edit::Mention,
	Edit::Shortcode,
	Edit::Hashtag,
	Edit::PickEmoji,
	Edit::InsertHashtag,
];

fn apply(doc: &mut Doc, edit: Edit, offset: u32) -> Result<(), HeadlessError> {
	caret(doc, offset)?;

	match edit {
		Edit::Mention => {
			doc.type_text(" @gr")?;
			doc.run_named("insertMention", &json!({ "id": "3", "label": "Grace" }))?;
		}
		Edit::Shortcode => {
			doc.type_text(" :rocket:")?;
		}
		Edit::Hashtag => {
			doc.type_text(" #wasm ")?;
		}
		Edit::PickEmoji => {
			doc.type_text(" :tad")?;
			doc.run_named("setEmoji", &json!({ "name": "tada" }))?;
		}
		Edit::InsertHashtag => {
			doc.type_text(" ")?;
			doc.run_named("insertHashtag", &json!({ "tag": "rust" }))?;
		}
	}

	Ok(())
}

/// Every pair of edits, made at the same place or at different places on
/// two peers before either sees the other's, converges to one document
/// holding both edits' atoms and tags.
#[test]
fn concurrent_atom_edits_converge() -> Result<(), HeadlessError> {
	for first_edit in EDITS {
		for second_edit in EDITS {
			for (first_at, second_at) in [(5, 5), (0, 11), (11, 0)] {
				let (mut first, mut second) = peers("Hello world")?;
				apply(&mut first, first_edit, first_at)?;
				apply(&mut second, second_edit, second_at)?;
				sync(&mut first, &mut second)?;

				let case = format!("{first_edit:?}@{first_at} vs {second_edit:?}@{second_at}");
				assert_eq!(content(&first), content(&second), "{case}");
				assert_eq!(
					first.export(Format::Html),
					second.export(Format::Html),
					"{case}"
				);
				assert_eq!(
					first.query("hashtags", &json!({}))?,
					second.query("hashtags", &json!({}))?,
					"{case}"
				);
				assert!(first.validate()?.is_valid(), "{case}");

				for edit in [first_edit, second_edit] {
					let html = first.export(Format::Html);
					let expected = match edit {
						Edit::Mention => "data-label=\"Grace\"",
						Edit::Shortcode => "data-name=\"rocket\"",
						Edit::Hashtag => "data-tag=\"wasm\"",
						Edit::PickEmoji => "data-name=\"tada\"",
						Edit::InsertHashtag => "data-tag=\"rust\"",
					};
					assert!(html.contains(expected), "{case}: {html}");
				}
			}
		}
	}

	Ok(())
}

#[test]
fn undoing_an_atom_on_one_peer_removes_it_on_both() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers("Hi")?;
	caret(&mut first, 2)?;
	first.type_text(" @a")?;
	first.run_named("insertMention", &json!({ "id": "1", "label": "Ann" }))?;
	apply(&mut second, Edit::Hashtag, 0)?;
	sync(&mut first, &mut second)?;
	assert!(first.export(Format::Html).contains("data-label=\"Ann\""));

	assert!(first.undo()?);
	sync(&mut first, &mut second)?;
	assert_eq!(content(&first), content(&second));
	assert!(
		!second
			.export(Format::Html)
			.contains("data-type=\"mention\"")
	);
	assert_eq!(
		second.query("hashtags", &json!({}))?,
		json!(["wasm"]),
		"the other peer's edit stays"
	);

	assert!(first.redo()?);
	sync(&mut first, &mut second)?;
	assert_eq!(content(&first), content(&second));
	assert!(second.export(Format::Html).contains("data-label=\"Ann\""));
	Ok(())
}

#[test]
fn a_late_joiner_sees_every_atom() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers("")?;
	apply(&mut first, Edit::Mention, 0)?;
	apply(&mut second, Edit::Shortcode, 0)?;
	sync(&mut first, &mut second)?;

	let late = engine(3).from_yjs(&second.encode_state()?, 3)?;
	assert_eq!(content(&late), content(&first));
	assert!(late.validate()?.is_valid());
	Ok(())
}

// Server-side rendering.

#[cfg(feature = "ssr")]
mod ssr {
	use canvist_headless::Format;
	use canvist_headless::HeadlessError;
	use canvist_headless::Renderer;
	use serde_json::json;

	use super::engine;

	fn rendered() -> Result<canvist_headless::Doc, HeadlessError> {
		engine(1).from_json(
			&json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
			{ "type": "mention", "attrs": { "id": "7", "label": "Ada" } },
			{ "type": "text", "text": " " },
			{ "type": "mention", "attrs": { "id": "u1" } },
			{ "type": "text", "text": " " },
			{ "type": "emoji", "attrs": { "name": "smile", "emoji": "😄" } },
			{ "type": "emoji", "attrs": { "name": "wave" } },
			{ "type": "text", "text": " " },
			{ "type": "text", "text": "#team", "marks": [{ "type": "hashtag", "attrs": { "tag": "team" } }] }
		] }] }),
		)
	}

	#[test]
	fn atoms_render_as_readable_text() -> Result<(), HeadlessError> {
		let doc = rendered()?;
		let html = Renderer::new().html(&doc, 600.0).html;

		assert!(
			html.contains(
				"<span class=\"cv-e cv-e-mention\" dir=\"auto\" data-type=\"mention\">@Ada</span>"
			),
			"{html}"
		);
		assert!(
			html.contains(
				"<span class=\"cv-e cv-e-emoji\" dir=\"auto\" data-type=\"emoji\">😄</span>"
			),
			"an emoji is its own character, which assistive technology names: {html}"
		);
		assert!(
			html.contains("data-type=\"emoji\">:wave:</span>"),
			"an emoji without its character reads as its shortcode: {html}"
		);
		assert!(
			html.contains("<span class=\"cv-m cv-m-hashtag\">#team</span>"),
			"{html}"
		);
		assert_eq!(doc.export(Format::Text), "@Ada @u1 😄:wave: #team");
		Ok(())
	}

	/// Regression: a mention without a label drew a bare `@`, leaving it
	/// with no readable name, where every export shows `@` and its id.
	#[test]
	fn a_mention_without_a_label_renders_its_id() -> Result<(), HeadlessError> {
		let doc = rendered()?;
		let html = Renderer::new().html(&doc, 600.0).html;

		assert!(html.contains("data-type=\"mention\">@u1</span>"), "{html}");
		let svg = Renderer::new().svg(&doc, 600.0)?;
		assert!(svg.contains("@u1"), "the canvas draws the id too: {svg}");
		Ok(())
	}
}
