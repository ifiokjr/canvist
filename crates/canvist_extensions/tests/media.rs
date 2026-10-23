//! Media blocks: images, videos, audio, and file attachments, inserted by
//! command and typing, and round-tripped through HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Value;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

fn typist(script: &str) -> Typist {
	let mut typist = Typist::with(support::editor_with(canvist_extensions::extras()));
	typist.type_script(script);
	typist
}

fn run(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	editor
		.run_named(name, &args, &mut Env::headless(Duration::ZERO))
		.expect("commands run")
		.is_some()
}

fn doc(typist: &Typist) -> Json {
	typist.snapshot()["doc"].clone()
}

/// HTML through the clipboard codecs and back.
fn html_round_trip(html: &str) -> String {
	let typist = typist("");
	let schema = typist.editor.schema();
	clipboard::to_html(schema, &clipboard::from_html(schema, html))
}

#[test]
fn set_image_inserts_an_image_block_with_its_attributes() {
	let mut typist = typist("Above");

	assert!(run(
		&mut typist.editor,
		"setImage",
		&[
			("src", Value::from("cat.png")),
			("alt", Value::from("A cat")),
			("width", Value::Int(320))
		]
	));
	assert_eq!(
		doc(&typist)[1],
		json!({ "type": "image", "attrs": { "alt": "A cat", "src": "cat.png", "width": 320 } })
	);
}

#[test]
fn typing_markdown_image_syntax_inserts_an_image() {
	let typist = typist("![[A cat](cat.png \"Meow\")");

	assert_eq!(
		doc(&typist)[0],
		json!({ "type": "image", "attrs": { "alt": "A cat", "src": "cat.png", "title": "Meow" } })
	);
}

#[test]
fn every_media_command_inserts_its_block() {
	let cases = [
		(
			"setYoutubeVideo",
			"src",
			"https://youtu.be/dQw4w9WgXcQ",
			"youtube",
		),
		(
			"setTwitchVideo",
			"src",
			"https://www.twitch.tv/videos/1",
			"twitch",
		),
		("setAudio", "src", "song.mp3", "audio"),
		("insertFile", "url", "report.pdf", "file"),
	];

	for (command, source, value, block) in cases {
		let mut typist = typist("");
		assert!(
			run(&mut typist.editor, command, &[(source, Value::from(value))]),
			"{command}"
		);
		assert_eq!(doc(&typist)[0]["type"], block, "{command}");
		assert_eq!(doc(&typist)[0]["attrs"][source], value, "{command}");
	}
}

#[test]
fn media_round_trips_through_html_as_tiptap_writes_it() {
	let cases = [
		"<img src=\"cat.png\" alt=\"A cat\" title=\"Meow\" width=\"320\">",
		"<div data-youtube-video=\"\"><iframe title=\"YouTube video\" \
		 src=\"https://www.youtube.com/embed/x\" width=\"640\" height=\"480\"></iframe></div>",
		"<div data-twitch-video=\"\"><iframe title=\"Twitch video\" \
		 src=\"https://player.twitch.tv/?video=1\" width=\"640\" height=\"480\"></iframe></div>",
		"<audio src=\"song.mp3\" controls loop></audio>",
		"<div data-type=\"file\" data-url=\"report.pdf\" data-file-name=\"Report\" \
		 data-file-type=\"application/pdf\" data-file-size=\"2048\"></div>",
	];

	for html in cases {
		assert_eq!(html_round_trip(html), html);
	}
}

#[test]
fn an_image_inside_a_paragraph_splits_it() {
	assert_eq!(
		html_round_trip("<p>before <img src=\"a.png\"> after</p>"),
		"<p>before</p><img src=\"a.png\"><p>after</p>"
	);
}

#[test]
fn other_iframes_are_dropped() {
	assert_eq!(
		html_round_trip("<p>x</p><iframe src=\"https://example.com\"></iframe>"),
		"<p>x</p>"
	);
}

#[test]
fn images_round_trip_through_markdown_and_other_media_become_links() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let markdown = "Look:\n\n![A cat](cat.png \"Meow\")";
	let image = clipboard::from_markdown(schema, markdown);

	assert_eq!(clipboard::to_markdown(schema, &image), markdown);

	let video = clipboard::from_html(
		schema,
		"<div data-youtube-video><iframe src=\"https://youtu.be/x\"></iframe></div>",
	);
	assert_eq!(
		clipboard::to_markdown(schema, &video),
		"[https://youtu.be/x](https://youtu.be/x)"
	);
}

#[test]
fn media_pasted_without_the_extension_is_dropped() {
	let plain = support::editor().schema().clone();
	let pasted = clipboard::from_html(&plain, "<p>a</p><img src=\"cat.png\"><p>b</p>");

	assert_eq!(clipboard::to_html(&plain, &pasted), "<p>a</p><p>b</p>");
}

#[test]
fn fractional_sizes_keep_their_fraction_and_whole_ones_drop_it() {
	let mut typist = typist("");
	assert!(run(
		&mut typist.editor,
		"setImage",
		&[
			("src", Value::from("cat.png")),
			("width", Value::Float(320.5)),
			("height", Value::Float(240.0))
		]
	));
	typist.type_script("[Mod-a]");
	let schema = typist.editor.schema().clone();
	let html = clipboard::to_html(&schema, &typist.editor.copy().expect("copies"));

	assert!(html.contains("width=\"320.5\" height=\"240\""), "{html}");
}

// Commands, typed handles, and their failures.

/// The whole document as a slice, for the clipboard writers.
fn whole(doc: &canvist_model::Document) -> canvist_core::Slice {
	let blocks = doc
		.children(None)
		.iter()
		.filter_map(|&id| doc.tree(id))
		.collect();
	canvist_core::Slice::new(blocks)
}

fn html(typist: &Typist) -> String {
	clipboard::to_html(typist.editor.schema(), &whole(typist.editor.state().doc()))
}

fn markdown(typist: &Typist) -> String {
	clipboard::to_markdown(typist.editor.schema(), &whole(typist.editor.state().doc()))
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

/// A StarterKit editor with every media extension and its typed handles.
struct Media {
	editor: Editor,
	image: canvist_extensions::MediaHandle,
	youtube: canvist_extensions::MediaHandle,
	twitch: canvist_extensions::MediaHandle,
	audio: canvist_extensions::MediaHandle,
	file: canvist_extensions::MediaHandle,
}

fn media() -> Media {
	let mut builder = Editor::builder()
		.extensions(canvist_extensions::StarterKit::default().extensions())
		.client(1);
	let image = builder.add(canvist_extensions::Image);
	let youtube = builder.add(canvist_extensions::Youtube);
	let twitch = builder.add(canvist_extensions::Twitch);
	let audio = builder.add(canvist_extensions::Audio);
	let file = builder.add(canvist_extensions::File);
	Media {
		editor: builder.build().expect("the media extensions build"),
		image,
		youtube,
		twitch,
		audio,
		file,
	}
}

#[test]
fn typed_handles_insert_every_media_block() {
	let Media {
		editor,
		image,
		youtube,
		twitch,
		audio,
		file,
	} = media();
	let mut typist = Typist::with(editor);
	let attrs = Attrs::new()
		.with("alt", "A cat")
		.with("width", Value::Null)
		.with("nonsense", true);
	let commands = [
		image.insert("cat.png", &attrs),
		youtube.insert(
			"https://youtu.be/x",
			&Attrs::new().with("start", 30).with("width", 640),
		),
		twitch.insert(
			"https://www.twitch.tv/videos/1",
			&Attrs::new().with("muted", true),
		),
		audio.insert("song.mp3", &Attrs::new().with("controls", false)),
		file.insert("r.pdf", &Attrs::new().with("fileName", "Report")),
	];

	for command in &commands {
		assert!(
			typist
				.editor
				.run(command, &mut env())
				.expect("runs")
				.is_some()
		);
	}

	let attrs: Vec<Json> = doc(&typist)
		.as_array()
		.expect("blocks")
		.iter()
		.map(|block| json!([block["type"], block["attrs"]]))
		.collect();
	assert_eq!(
		attrs,
		[
			json!(["image", { "alt": "A cat", "src": "cat.png" }]),
			json!(["youtube", { "src": "https://youtu.be/x", "start": 30 }]),
			json!(["twitch", { "muted": true, "src": "https://www.twitch.tv/videos/1" }]),
			json!(["audio", { "controls": false, "src": "song.mp3" }]),
			json!(["file", { "fileName": "Report", "url": "r.pdf" }]),
			json!(["paragraph", null]),
		],
		"unknown attributes and ones equal to their default are dropped"
	);
}

#[test]
fn media_commands_insert_where_the_caret_is() {
	let mut typist = typist("ab←");
	assert!(run(
		&mut typist.editor,
		"setImage",
		&[("src", Value::from("x.png"))]
	));

	assert_eq!(
		html(&typist),
		"<p>a</p><img src=\"x.png\"><p>b</p>",
		"in the middle of a paragraph, the image splits it"
	);
	assert_eq!(
		typist.snapshot()["selection"],
		"caret 2@0",
		"the caret is after it"
	);

	let mut typist = self::typist("ab←←");
	run(
		&mut typist.editor,
		"setImage",
		&[("src", Value::from("x.png"))],
	);
	assert_eq!(
		html(&typist),
		"<img src=\"x.png\"><p>ab</p>",
		"at the start, it goes before"
	);

	let mut typist = self::typist("one two[Shift-ArrowLeft][Shift-ArrowLeft][Shift-ArrowLeft]");
	run(
		&mut typist.editor,
		"setImage",
		&[("src", Value::from("x.png"))],
	);
	assert_eq!(
		html(&typist),
		"<p>one </p><img src=\"x.png\"><p></p>",
		"a selection is replaced"
	);
}

#[test]
fn media_commands_check_with_can_and_change_nothing() {
	let mut typist = typist("Hello");
	let before = typist.snapshot();
	let command = typist
		.editor
		.command_named("setAudio", &Attrs::new().with("src", "a.mp3"))
		.expect("known");

	assert!(typist.editor.can(&command, &mut env()).expect("checks"));
	assert_eq!(typist.snapshot(), before);
}

fn invalid(editor: &mut Editor, name: &str, args: &Attrs) {
	match editor.run_named(name, args, &mut env()) {
		Err(canvist_core::EditorError::InvalidArgs { name: failed, .. }) => {
			assert_eq!(failed, name);
		}
		other => panic!("{name} {args:?}: expected invalid arguments, got {other:?}"),
	}
}

#[test]
fn invalid_media_arguments_are_reported() {
	let mut typist = typist("");
	let before = typist.snapshot();
	let editor = &mut typist.editor;

	invalid(editor, "setImage", &Attrs::new());
	invalid(editor, "setImage", &Attrs::new().with("src", Value::Null));
	invalid(editor, "setImage", &Attrs::new().with("src", 7));
	invalid(
		editor,
		"setImage",
		&Attrs::new().with("src", "a").with("width", "wide"),
	);
	invalid(
		editor,
		"setImage",
		&Attrs::new().with("src", "a").with("alt", 3),
	);
	invalid(
		editor,
		"setYoutubeVideo",
		&Attrs::new()
			.with("src", "https://youtu.be/x")
			.with("start", "0:30"),
	);
	invalid(
		editor,
		"setTwitchVideo",
		&Attrs::new()
			.with("src", "https://twitch.tv/a")
			.with("muted", "yes"),
	);
	invalid(
		editor,
		"setAudio",
		&Attrs::new().with("src", "a.mp3").with("loop", 1),
	);
	invalid(
		editor,
		"insertFile",
		&Attrs::new().with("fileName", "r.pdf"),
	);
	invalid(
		editor,
		"insertFile",
		&Attrs::new().with("url", "r.pdf").with("fileSize", "big"),
	);

	assert_eq!(typist.snapshot(), before, "failures change nothing");
}

#[test]
fn media_handles_fail_on_a_schema_without_their_type() {
	let Media { image, file, .. } = media();
	let mut plain = Typist::new();

	for command in [
		image.insert("a.png", &Attrs::new()),
		file.insert("r.pdf", &Attrs::new()),
	] {
		let error = plain
			.editor
			.run(&command, &mut env())
			.expect_err("no such type");
		assert!(
			matches!(error, canvist_core::EditorError::MissingType { .. }),
			"{error:?}"
		);
	}
}

/// Regression: video commands took any URL, and the HTML reader took any
/// iframe whose `src` merely contained `youtube` or `twitch.tv`, so a
/// `javascript:` URL could reach an exported `<iframe src>`.
#[test]
fn videos_only_embed_their_own_sites() {
	let accepted = [
		("setYoutubeVideo", "https://www.youtube.com/watch?v=x"),
		("setYoutubeVideo", "https://youtu.be/x"),
		("setYoutubeVideo", "http://m.youtube.com/embed/x"),
		("setYoutubeVideo", "//www.youtube-nocookie.com/embed/x"),
		("setYoutubeVideo", "HTTPS://YOUTUBE.COM/embed/x"),
		("setTwitchVideo", "https://player.twitch.tv/?channel=a"),
		("setTwitchVideo", "https://clips.twitch.tv/a"),
		("setTwitchVideo", "https://twitch.tv:443/videos/1"),
	];
	let refused = [
		("setYoutubeVideo", "javascript:alert('youtube')"),
		("setYoutubeVideo", "data:text/html,youtube.com"),
		("setYoutubeVideo", "https://example.com/?v=youtube.com"),
		("setYoutubeVideo", "https://youtube.com.example.com/x"),
		("setYoutubeVideo", "https://notyoutube.com/x"),
		("setYoutubeVideo", "https://youtube.com@example.com/x"),
		("setYoutubeVideo", "youtube.com/watch?v=x"),
		("setYoutubeVideo", ""),
		("setYoutubeVideo", "https://www.twitch.tv/videos/1"),
		("setTwitchVideo", "https://www.youtube.com/embed/x"),
		("setTwitchVideo", "javascript:twitch.tv"),
	];

	for (command, src) in accepted {
		let mut typist = typist("");
		assert!(
			run(&mut typist.editor, command, &[("src", Value::from(src))]),
			"{command} {src}"
		);
	}

	for (command, src) in refused {
		let mut typist = typist("");
		let can = typist
			.editor
			.command_named(command, &Attrs::new().with("src", src))
			.expect("known");
		assert!(
			!typist.editor.can(&can, &mut env()).expect("checks"),
			"{command} {src}"
		);
		assert!(
			!run(&mut typist.editor, command, &[("src", Value::from(src))]),
			"{command} {src}"
		);
		assert_eq!(
			doc(&typist),
			json!([{ "type": "paragraph" }]),
			"{command} {src}"
		);
	}

	assert_eq!(
		html_round_trip(
			"<p>x</p><div data-youtube-video><iframe src=\"javascript:alert('youtube')\"></iframe></div>\
			 <div data-twitch-video><iframe src=\"https://evil.example/twitch.tv\"></iframe></div>"
		),
		"<p>x</p>",
		"iframes from other sites are dropped"
	);

	let typist = typist("");
	let loaded = canvist_model::Document::from_json(
		typist.editor.schema().clone(),
		1,
		&json!({ "type": "doc", "content": [{ "type": "youtube", "attrs": { "src": "javascript:alert(1)" } }] }),
	)
	.expect("JSON loads whatever it holds");
	assert_eq!(
		clipboard::to_html(typist.editor.schema(), &whole(&loaded)),
		"<div data-youtube-video=\"\"><iframe title=\"YouTube video\"></iframe></div>",
		"a video from JSON with a bad source is written without it"
	);
	assert_eq!(
		clipboard::to_markdown(typist.editor.schema(), &whole(&loaded)),
		"",
		"and Markdown leaves it out rather than write a `javascript:` link"
	);
}

// Typing.

#[test]
fn typed_image_syntax_takes_optional_alt_and_title() {
	let cases = [
		("![[](x.png)", json!({ "src": "x.png" })),
		(
			"![[A b](x.png 'Single')",
			json!({ "alt": "A b", "src": "x.png", "title": "Single" }),
		),
		(
			"![[c](https://e.com/x.png?a=1)",
			json!({ "alt": "c", "src": "https://e.com/x.png?a=1" }),
		),
	];

	for (script, attrs) in cases {
		let typist = typist(script);
		assert_eq!(
			doc(&typist)[0],
			json!({ "type": "image", "attrs": attrs }),
			"{script}"
		);
	}

	let typist = typist("x![[a](b.png)");
	assert_eq!(
		doc(&typist),
		json!([{ "type": "paragraph", "content": [{ "type": "text", "text": "x![a](b.png)" }] }]),
		"the syntax needs a space or the line start before it"
	);
	let typist = self::typist("![[a](b c.png)");
	assert_eq!(
		doc(&typist)[0]["type"],
		"paragraph",
		"a source has no spaces"
	);
}

#[test]
fn a_typed_image_after_text_splits_the_paragraph() {
	let typist = typist("see ![[a](b.png)");

	assert_eq!(
		html(&typist),
		"<p>see </p><img src=\"b.png\" alt=\"a\"><p></p>"
	);
}

#[test]
fn backspace_right_after_a_typed_image_restores_the_syntax() {
	let mut typist = typist("![[a](b.png)");
	assert_eq!(doc(&typist)[0]["type"], "image");

	typist.type_script("⌫");
	assert_eq!(
		doc(&typist),
		json!([{ "type": "paragraph", "content": [{ "type": "text", "text": "![a](b.png)" }] }])
	);
}

#[test]
fn undo_and_redo_insert_and_remove_media() {
	let mut typist = typist("Hi");
	let before = typist.snapshot();
	let mut later = Env::headless(Duration::from_secs(60));
	let command = typist
		.editor
		.command_named("insertFile", &Attrs::new().with("url", "r.pdf"))
		.expect("known");
	typist.editor.run(&command, &mut later).expect("runs");
	let after = typist.snapshot();

	typist.editor.undo(&mut later).expect("undoes");
	assert_eq!(typist.snapshot(), before);
	typist.editor.redo(&mut later).expect("redoes");
	assert_eq!(typist.snapshot(), after);
}

// Serialisation.

#[test]
fn every_media_block_round_trips_through_json() {
	let typist = typist("");
	let schema = typist.editor.schema().clone();
	let json = json!({ "type": "doc", "content": [
		{ "type": "image", "attrs": { "src": "a.png", "alt": "A", "title": "T", "width": 10.5, "height": 20 } },
		{ "type": "youtube", "attrs": { "src": "https://youtu.be/x", "start": 30, "width": 400, "height": 300 } },
		{ "type": "twitch", "attrs": { "src": "https://twitch.tv/a", "autoplay": true, "muted": true } },
		{ "type": "audio", "attrs": { "src": "a.mp3", "controls": false, "loop": true } },
		{ "type": "file", "attrs": { "url": "r.pdf", "fileName": "R", "fileType": "application/pdf", "fileSize": 9 } },
		{ "type": "paragraph" }
	] });
	let doc = canvist_model::Document::from_json(schema.clone(), 1, &json).expect("valid JSON");
	let restored =
		canvist_model::Document::from_json(schema, 1, &doc.to_json()).expect("its own JSON reads");

	assert_eq!(restored.to_json(), doc.to_json());
	let mut written = doc.to_json();
	for block in written["content"].as_array_mut().expect("blocks") {
		if let Some(attrs) = block["attrs"].as_object_mut() {
			attrs.remove("id");
		}
	}
	assert_eq!(
		written["content"][0], json["content"][0],
		"attributes are kept as given"
	);
	assert_eq!(written["content"][4], json["content"][4]);
}

#[test]
fn audio_flags_and_file_cards_round_trip_through_html() {
	let cases = [
		"<audio src=\"a.mp3\"></audio>",
		"<audio src=\"a.mp3\" controls autoplay loop muted></audio>",
		"<div data-type=\"file\" data-url=\"r.pdf\" data-file-size=\"0\"></div>",
		"<img src=\"a.png\" alt=\"Say &quot;cheese&quot; &amp; smile\" width=\"10.5\" \
		 height=\"20\">",
	];

	for html in cases {
		assert_eq!(html_round_trip(html), html);
	}

	assert_eq!(
		html_round_trip("<div data-type=\"file\" data-url=\"r.pdf\"></div>"),
		"<div data-type=\"file\" data-url=\"r.pdf\" data-file-size=\"0\"></div>",
		"read attributes take their defaults, which are then written"
	);
}

/// Regression: a file without a name wrote `[](r.pdf)`, an empty link that
/// reads back as nothing, so the attachment vanished.
#[test]
fn media_write_to_markdown_and_images_read_back() {
	let mut typist = typist("");
	for (command, source, value) in [
		("setImage", "src", "a.png"),
		("setYoutubeVideo", "src", "https://youtu.be/x"),
		("setAudio", "src", "a.mp3"),
		("insertFile", "url", "r.pdf"),
	] {
		run(&mut typist.editor, command, &[(source, Value::from(value))]);
	}
	let written = markdown(&typist);

	assert_eq!(
		written,
		"![](a.png)\n\n[https://youtu.be/x](https://youtu.be/x)\n\n[a.mp3](a.mp3)\n\n[r.pdf](r.pdf)"
	);
	let schema = typist.editor.schema();
	let read = clipboard::from_markdown(schema, &written);
	assert_eq!(
		clipboard::to_markdown(schema, &read),
		written,
		"Markdown is stable, though videos, audio, and files come back as links"
	);
	assert_eq!(
		clipboard::to_html(schema, &read),
		"<img src=\"a.png\"><p><a href=\"https://youtu.be/x\">https://youtu.be/x</a></p><p><a \
		 href=\"a.mp3\">a.mp3</a></p><p><a href=\"r.pdf\">r.pdf</a></p>"
	);
}

#[test]
fn video_frames_have_a_title_for_assistive_technology() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let html = clipboard::to_html(
		schema,
		&clipboard::from_html(
			schema,
			"<div data-youtube-video><iframe src=\"https://youtu.be/x\"></iframe></div>\
			 <div data-twitch-video><iframe src=\"https://twitch.tv/a\"></iframe></div>",
		),
	);

	assert_eq!(
		html,
		"<div data-youtube-video=\"\"><iframe title=\"YouTube video\" src=\"https://youtu.be/x\" \
		 width=\"640\" height=\"480\"></iframe></div><div data-twitch-video=\"\"><iframe \
		 title=\"Twitch video\" src=\"https://twitch.tv/a\" width=\"640\" \
		 height=\"480\"></iframe></div>"
	);
}

#[test]
fn pasted_html_media_lands_in_the_document() {
	let mut typist = typist("ab←");
	let schema = typist.editor.schema().clone();
	let slice = clipboard::from_html(
		&schema,
		"<img src=\"a.png\" alt=\"A\"><audio src=\"s.mp3\" controls></audio>",
	);
	typist.editor.paste(slice, &mut env()).expect("pastes");
	support::assert_valid(typist.editor.state());

	assert_eq!(
		html(&typist),
		"<p>a</p><img src=\"a.png\" alt=\"A\"><audio src=\"s.mp3\" controls></audio><p>b</p>"
	);
}
