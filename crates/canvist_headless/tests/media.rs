//! Media blocks through the headless API: Yjs peers inserting them
//! concurrently converge with every attribute intact, failures carry
//! stable error codes, and server-side rendering reserves their space.

#![cfg(feature = "full")]

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::ErrorCode;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use serde_json::Value as Json;
use serde_json::json;

const MEDIA: [&str; 5] = ["image", "youtube", "twitch", "audio", "file"];

fn engine(client: u32) -> Engine {
	Engine::named(&MEDIA)
		.expect("the media extensions register")
		.with_client(client)
}

/// Every media command with arguments that set non-default attributes of
/// each kind: text, numbers with and without a fraction, and flags.
fn inserts() -> [(&'static str, Json); 5] {
	[
		(
			"setImage",
			json!({ "src": "cat.png", "alt": "A cat", "width": 320.5, "height": 240 }),
		),
		(
			"setYoutubeVideo",
			json!({ "src": "https://youtu.be/x", "start": 30 }),
		),
		(
			"setTwitchVideo",
			json!({ "src": "https://twitch.tv/a", "autoplay": true, "muted": true }),
		),
		(
			"setAudio",
			json!({ "src": "a.mp3", "controls": false, "loop": true }),
		),
		(
			"insertFile",
			json!({ "url": "r.pdf", "fileName": "Report", "fileSize": 2048 }),
		),
	]
}

fn caret(doc: &mut Doc, path: u32, offset: u32) -> Result<(), HeadlessError> {
	let position = Position::new([path], offset);
	doc.select_range(&position, &position)
}

#[test]
fn failures_carry_stable_codes() -> Result<(), HeadlessError> {
	let mut doc = engine(1).create()?;
	let before = doc.to_json();
	let code = |result: Result<bool, HeadlessError>| result.expect_err("fails").code();

	assert_eq!(
		code(doc.run_named("setImage", &json!({}))),
		ErrorCode::InvalidArgs
	);
	assert_eq!(
		code(doc.run_named("setImage", &json!({ "src": null }))),
		ErrorCode::InvalidArgs
	);
	assert_eq!(
		code(doc.run_named("setImage", &json!({ "src": "a", "width": "wide" }))),
		ErrorCode::InvalidArgs
	);
	assert_eq!(
		code(doc.run_named("setAudio", &json!({ "src": "a", "muted": "no" }))),
		ErrorCode::InvalidArgs
	);
	assert_eq!(
		code(doc.run_named("insertFile", &json!({}))),
		ErrorCode::InvalidArgs
	);
	assert!(
		!doc.run_named("setYoutubeVideo", &json!({ "src": "javascript:alert(1)" }))?,
		"a bad video URL is refused, not an error"
	);
	assert_eq!(doc.to_json(), before, "failures change nothing");

	let mut plain = Engine::starter_kit().create()?;
	assert_eq!(
		code(plain.run_named("setImage", &json!({ "src": "a" }))),
		ErrorCode::UnknownName
	);
	assert_eq!(
		Engine::named(&["video"])
			.expect_err("an unknown extension")
			.code(),
		ErrorCode::Config
	);
	Ok(())
}

#[test]
fn media_exports_in_every_format() -> Result<(), HeadlessError> {
	let mut doc = engine(1).create()?;

	for (command, args) in inserts() {
		assert!(doc.run_named(command, &args)?, "{command}");
	}

	assert_eq!(
		doc.export(Format::Html),
		"<img src=\"cat.png\" alt=\"A cat\" width=\"320.5\" height=\"240\"><div \
		 data-youtube-video=\"\"><iframe title=\"YouTube video\" \
		 src=\"https://youtu.be/x\"></iframe></div><div data-twitch-video=\"\"><iframe \
		 title=\"Twitch video\" src=\"https://twitch.tv/a\"></iframe></div><audio src=\"a.mp3\" \
		 loop></audio><div data-type=\"file\" data-url=\"r.pdf\" data-file-name=\"Report\" \
		 data-file-size=\"2048\"></div><p></p>"
	);
	assert_eq!(
		doc.export(Format::Markdown),
		"![A cat](cat.png)\n\n[https://youtu.be/x](https://youtu.be/x)\n\n[https://twitch.tv/a](https://twitch.tv/a)\n\n[a.mp3](a.mp3)\n\n[Report](r.pdf)"
	);
	assert_eq!(doc.export(Format::Text), "", "media blocks have no text");

	let restored = engine(1).parse(Format::Json, &doc.export(Format::Json))?;
	assert_eq!(restored.to_json(), doc.to_json());
	Ok(())
}

// Collaboration.

fn peers() -> Result<(Doc, Doc), HeadlessError> {
	let mut first = engine(1).parse(Format::Text, "one\ntwo")?;
	first.share(1)?;
	let second = engine(2).from_yjs(&first.encode_state()?, 2)?;
	Ok((first, second))
}

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

/// Block types and attributes without block ids, which peers allocate
/// differently.
fn blocks(doc: &Doc) -> Json {
	let mut content = doc.to_json()["content"].clone();

	for block in content.as_array_mut().into_iter().flatten() {
		if let Some(attrs) = block.get_mut("attrs").and_then(Json::as_object_mut) {
			attrs.remove("id");
		}
	}

	content
}

/// Every pair of media inserts, at the same caret and at different ones on
/// two peers before either sees the other's, converges with both blocks
/// and all their attributes.
#[test]
fn concurrent_media_inserts_converge() -> Result<(), HeadlessError> {
	let places = [((0, 3), (0, 3)), ((0, 0), (1, 3)), ((1, 3), (0, 1))];

	for (first_command, first_args) in inserts() {
		for (second_command, second_args) in inserts() {
			for (first_at, second_at) in places {
				let (mut first, mut second) = peers()?;
				caret(&mut first, first_at.0, first_at.1)?;
				caret(&mut second, second_at.0, second_at.1)?;
				assert!(first.run_named(first_command, &first_args)?);
				assert!(second.run_named(second_command, &second_args)?);
				sync(&mut first, &mut second)?;

				let case =
					format!("{first_command}@{first_at:?} vs {second_command}@{second_at:?}");
				assert_eq!(blocks(&first), blocks(&second), "{case}");
				assert!(first.validate()?.is_valid(), "{case}");

				for args in [&first_args, &second_args] {
					let source = args.get("src").or_else(|| args.get("url"));
					let found = blocks(&first)
						.as_array()
						.into_iter()
						.flatten()
						.filter(|block| {
							let attrs = &block["attrs"];
							attrs.get("src").or_else(|| attrs.get("url")) == source
						})
						.count();
					assert!(found >= 1, "{case}: {source:?} is in both documents");
				}
			}
		}
	}

	Ok(())
}

#[test]
fn media_attributes_survive_yjs_exactly() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers()?;
	caret(&mut first, 1, 3)?;

	for (command, args) in inserts() {
		first.run_named(command, &args)?;
	}

	sync(&mut first, &mut second)?;
	let late = engine(3).from_yjs(&second.encode_state()?, 3)?;

	assert_eq!(blocks(&second), blocks(&first));
	assert_eq!(blocks(&late), blocks(&first));
	assert_eq!(late.export(Format::Html), first.export(Format::Html));
	assert_eq!(
		blocks(&late)[2]["attrs"],
		json!({ "alt": "A cat", "height": 240, "src": "cat.png", "width": 320.5 }),
		"numbers keep their kind and flags their value"
	);
	assert_eq!(
		blocks(&late)[4]["attrs"],
		json!({ "autoplay": true, "muted": true, "src": "https://twitch.tv/a" })
	);
	Ok(())
}

#[test]
fn undoing_a_media_insert_on_one_peer_removes_it_on_both() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers()?;
	caret(&mut first, 0, 3)?;
	first.run_named("setImage", &json!({ "src": "a.png" }))?;
	caret(&mut second, 1, 3)?;
	second.type_text("!")?;
	sync(&mut first, &mut second)?;
	assert!(second.export(Format::Html).contains("<img src=\"a.png\">"));

	assert!(first.undo()?);
	sync(&mut first, &mut second)?;
	assert_eq!(blocks(&first), blocks(&second));
	assert_eq!(second.export(Format::Html), "<p>one</p><p>two!</p>");
	Ok(())
}

// Server-side rendering.

#[cfg(feature = "ssr")]
#[test]
fn server_rendering_reserves_each_media_slot() -> Result<(), HeadlessError> {
	let mut doc = engine(1).create()?;

	for (command, args) in inserts() {
		doc.run_named(command, &args)?;
	}

	let html = canvist_headless::Renderer::new().html(&doc, 600.0).html;

	for (name, height) in [
		("image", "240"),
		("youtube", "200"),
		("twitch", "200"),
		("audio", "56"),
		("file", "64"),
	] {
		let class = format!("cv-t-{name}\"");
		let start = html
			.find(&class)
			.unwrap_or_else(|| panic!("no {name} slot in {html}"));
		let tag = &html[start
			..html[start..]
				.find('>')
				.map_or(html.len(), |end| start + end)];
		assert!(tag.contains(&format!("height:{height}px")), "{name}: {tag}");
	}

	Ok(())
}

#[cfg(feature = "ssr")]
#[test]
fn server_rendered_images_carry_their_alt_text() -> Result<(), HeadlessError> {
	let mut doc = engine(1).create()?;
	doc.run_named(
		"setImage",
		&json!({ "src": "cat.png", "alt": "A cat & a dog" }),
	)?;
	doc.run_named("setImage", &json!({ "src": "rule.png", "alt": "" }))?;

	let html = canvist_headless::Renderer::new().html(&doc, 600.0).html;

	assert!(
		html.contains("role=\"img\" aria-label=\"A cat &amp; a dog\""),
		"the image is named by its alt text, escaped: {html}"
	);
	assert_eq!(
		html.matches("role=\"img\"").count(),
		1,
		"an empty alt marks a decorative image, which gets no name: {html}"
	);

	Ok(())
}
