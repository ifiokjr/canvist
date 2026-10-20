//! Media blocks: images, videos, audio, and file attachments, inserted by
//! command and typing, and round-tripped through HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
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
		"<div data-youtube-video=\"\"><iframe src=\"https://www.youtube.com/embed/x\" \
		 width=\"640\" height=\"480\"></iframe></div>",
		"<div data-twitch-video=\"\"><iframe src=\"https://player.twitch.tv/?video=1\" \
		 width=\"640\" height=\"480\"></iframe></div>",
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
