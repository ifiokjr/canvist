//! Property tests for media blocks: documents of random images, videos,
//! audio, and files survive JSON, HTML, Markdown, and history, typed image
//! syntax keeps what was typed, and only YouTube's and Twitch's URLs ever
//! become videos.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::Slice;
use canvist_extensions::StarterKit;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Document;
use canvist_model::Value;
use proptest::prelude::*;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

/// A StarterKit editor with every media extension, and nothing that
/// rewrites typed text.
fn editor() -> Editor {
	let mut builder = Editor::builder()
		.extensions(StarterKit::default().extensions())
		.client(1);
	builder.add(canvist_extensions::Image);
	builder.add(canvist_extensions::Youtube);
	builder.add(canvist_extensions::Twitch);
	builder.add(canvist_extensions::Audio);
	builder.add(canvist_extensions::File);
	builder.build().expect("the media extensions build")
}

fn env(seconds: u64) -> Env<'static> {
	Env::headless(Duration::from_secs(seconds))
}

fn whole(doc: &Document) -> Slice {
	let blocks = doc
		.children(None)
		.iter()
		.filter_map(|&id| doc.tree(id))
		.collect();
	Slice::new(blocks)
}

/// The blocks' types and attributes, without block ids.
fn blocks(doc: &Document) -> Json {
	let mut content = doc.to_json()["content"].clone();

	for block in content.as_array_mut().into_iter().flatten() {
		if let Some(attrs) = block.get_mut("attrs").and_then(Json::as_object_mut) {
			attrs.remove("id");
		}
	}

	content
}

/// Text any attribute may hold, including what HTML must escape.
fn text() -> impl Strategy<Value = String> {
	"[a-zA-Z0-9 .,&<>\"'=/-]{1,12}"
}

/// A URL-ish source without spaces.
fn source() -> impl Strategy<Value = String> {
	"[a-z0-9]{1,8}\\.(png|mp3|pdf)"
}

/// A size: a whole number or one with a fraction.
fn size() -> impl Strategy<Value = Json> {
	prop_oneof![
		(1_u32..2_000).prop_map(|size| json!(size)),
		(1_u32..2_000).prop_map(|size| json!(f64::from(size) + 0.5)),
	]
}

fn video_source() -> impl Strategy<Value = (&'static str, String)> {
	prop_oneof![
		"[a-zA-Z0-9_-]{4,11}"
			.prop_map(|id| ("youtube", format!("https://www.youtube.com/embed/{id}"))),
		"[a-zA-Z0-9_-]{4,11}".prop_map(|id| ("youtube", format!("https://youtu.be/{id}"))),
		"[a-z0-9_]{3,10}".prop_map(|channel| {
			(
				"twitch",
				format!("https://player.twitch.tv/?channel={channel}"),
			)
		}),
	]
}

/// One media block with every attribute HTML carries set, so reading its
/// HTML back fills nothing in from defaults.
fn media_block() -> impl Strategy<Value = Json> {
	prop_oneof![
		(source(), text(), text(), size(), size()).prop_map(|(src, alt, title, width, height)| {
			json!({ "type": "image", "attrs": { "src": src, "alt": alt, "title": title, "width": width, "height": height } })
		}),
		(video_source(), size(), size()).prop_map(|((kind, src), width, height)| {
			json!({ "type": kind, "attrs": { "src": src, "width": width, "height": height } })
		}),
		(source(), any::<[bool; 4]>()).prop_map(|(src, [controls, autoplay, looped, muted])| {
			json!({ "type": "audio", "attrs": { "src": src, "controls": controls, "autoplay": autoplay, "loop": looped, "muted": muted } })
		}),
		(source(), text(), "[a-z]{1,6}/[a-z]{1,6}", 0_u32..1_000_000).prop_map(|(url, name, media, size)| {
			json!({ "type": "file", "attrs": { "url": url, "fileName": name, "fileType": media, "fileSize": size } })
		}),
	]
}

fn document(blocks: Vec<Json>) -> Json {
	let mut content = blocks;
	content.push(json!({ "type": "paragraph" }));
	json!({ "type": "doc", "content": content })
}

/// `json` with every number as a float: HTML reads `320` back as `320.0`.
fn floats(json: Json) -> Json {
	match json {
		Json::Number(number) => json!(number.as_f64()),
		Json::Array(items) => Json::Array(items.into_iter().map(floats).collect()),
		Json::Object(fields) => {
			Json::Object(
				fields
					.into_iter()
					.map(|(key, value)| (key, floats(value)))
					.collect(),
			)
		}
		other => other,
	}
}

/// The JSON defaults HTML leaves at their values: a video's start and
/// Twitch's playback flags are not part of TipTap's HTML.
fn with_video_defaults(mut blocks: Json) -> Json {
	for block in blocks.as_array_mut().into_iter().flatten() {
		let defaults = match block["type"].as_str() {
			Some("youtube") => json!({ "start": 0 }),
			Some("twitch") => json!({ "autoplay": false, "muted": false }),
			_ => continue,
		};

		for (key, value) in defaults.as_object().into_iter().flatten() {
			block["attrs"][key] = value.clone();
		}
	}

	blocks
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(128))]

	#[test]
	fn media_documents_round_trip_through_json_and_html(media in prop::collection::vec(media_block(), 1..6)) {
		let editor = editor();
		let schema = editor.schema().clone();
		let doc = Document::from_json(schema.clone(), 1, &document(media)).expect("valid JSON");

		let restored = Document::from_json(schema.clone(), 1, &doc.to_json()).expect("its own JSON reads");
		prop_assert_eq!(restored.to_json(), doc.to_json());

		let html = clipboard::to_html(&schema, &whole(&doc));
		let read = clipboard::from_html(&schema, &html);
		prop_assert_eq!(clipboard::to_html(&schema, &read), html.clone());

		let mut pasted = editor;
		pasted.paste(read, &mut env(0)).expect("pastes");
		support::assert_valid(pasted.state());
		prop_assert_eq!(
			floats(with_video_defaults(blocks(pasted.state().doc()))),
			floats(with_video_defaults(blocks(&doc))),
			"HTML keeps every attribute it carries: {}",
			html
		);
	}

	#[test]
	fn images_round_trip_through_markdown(
		src in source(),
		alt in "[a-zA-Z0-9 ]{0,10}",
		title in proptest::option::of("[a-zA-Z0-9 ]{1,10}"),
	) {
		let schema = editor().schema().clone();
		let alt = alt.trim().to_owned();
		let title = title.map(|title| title.trim().to_owned()).filter(|title| !title.is_empty());
		let markdown = match &title {
			Some(title) => format!("![{alt}]({src} \"{title}\")"),
			None => format!("![{alt}]({src})"),
		};
		let read = clipboard::from_markdown(&schema, &markdown);

		prop_assert_eq!(clipboard::to_markdown(&schema, &read), markdown);
		let tree = &read.blocks()[0];
		prop_assert_eq!(tree.attrs.get("src").and_then(Value::as_str), Some(src.as_str()));
		prop_assert_eq!(tree.attrs.get("title").and_then(Value::as_str), title.as_deref());
	}

	#[test]
	fn typed_image_syntax_keeps_what_was_typed(
		src in source(),
		alt in "[a-zA-Z0-9]{0,6}( [a-zA-Z0-9]{1,6})?",
		title in proptest::option::of("[a-zA-Z0-9]{1,6}( [a-zA-Z0-9]{1,6})?"),
	) {
		let mut typist = Typist::with(editor());
		let syntax = match &title {
			Some(title) => format!("![[{alt}]({src} \"{title}\")"),
			None => format!("![[{alt}]({src})"),
		};
		typist.type_script(&syntax);
		let block = typist.snapshot()["doc"][0].clone();

		let mut attrs = json!({ "src": src });
		if !alt.is_empty() {
			attrs["alt"] = json!(alt);
		}
		if let Some(title) = title {
			attrs["title"] = json!(title);
		}
		prop_assert_eq!(block, json!({ "type": "image", "attrs": attrs }));

		typist.type_script("⌫");
		prop_assert_eq!(&typist.snapshot()["doc"][0]["type"], "paragraph", "Backspace restores the text");
	}

	#[test]
	fn inserting_media_undoes_and_redoes_exactly(
		steps in prop::collection::vec((0_usize..5, source(), "[a-z ]{0,6}"), 1..6),
	) {
		let mut editor = editor();
		let mut states = vec![editor.state().doc().to_json()];
		let commands = ["setImage", "setYoutubeVideo", "setTwitchVideo", "setAudio", "insertFile"];

		for (seconds, (index, src, typed)) in (1_u64..).zip(steps) {
			let name = commands[index];
			let src = match name {
				"setYoutubeVideo" => format!("https://youtu.be/{src}"),
				"setTwitchVideo" => format!("https://twitch.tv/{src}"),
				_ => src,
			};
			let key = if name == "insertFile" { "url" } else { "src" };
			let args = Attrs::new().with(key, src.as_str());
			let applied = editor.run_named(name, &args, &mut env(seconds * 60)).expect("runs");
			prop_assert!(applied.is_some(), "{} {}", name, src);
			states.push(editor.state().doc().to_json());

			if !typed.is_empty() {
				editor.insert_text(&typed, &mut env(seconds * 60 + 30)).expect("types");
				states.push(editor.state().doc().to_json());
			}
		}

		for expected in states.iter().rev().skip(1) {
			editor.undo(&mut env(100_000)).expect("undoes");
			prop_assert_eq!(&editor.state().doc().to_json(), expected);
		}

		for expected in states.iter().skip(1) {
			editor.redo(&mut env(100_000)).expect("redoes");
			prop_assert_eq!(&editor.state().doc().to_json(), expected);
		}
	}

	#[test]
	fn only_youtube_and_twitch_urls_become_videos(
		scheme in prop::sample::select(vec!["https://", "http://", "//", "javascript:", "data:", "", "ftp://"]),
		user in proptest::option::of("[a-z]{1,5}@"),
		subdomain in proptest::option::of("[a-z]{1,5}\\."),
		host in prop::sample::select(vec![
			"youtube.com", "youtu.be", "youtube-nocookie.com", "twitch.tv",
			"youtube.com.evil.test", "evilyoutube.com", "twitch.tv.evil.test", "example.com",
		]),
		path in "(/[a-z0-9?=&.]{0,10})?",
	) {
		let src = format!(
			"{scheme}{}{}{host}{path}",
			user.as_deref().unwrap_or_default(),
			subdomain.as_deref().unwrap_or_default()
		);
		let expected = match (scheme, host) {
			("https://" | "http://" | "//", "youtube.com" | "youtu.be" | "youtube-nocookie.com") => Some("youtube"),
			("https://" | "http://" | "//", "twitch.tv") => Some("twitch"),
			_ => None,
		};

		for (command, kind) in [("setYoutubeVideo", "youtube"), ("setTwitchVideo", "twitch")] {
			let mut editor = editor();
			let applied = editor
				.run_named(command, &Attrs::new().with("src", src.as_str()), &mut env(0))
				.expect("runs")
				.is_some();
			prop_assert_eq!(applied, expected == Some(kind), "{} {}", command, src);
		}

		let schema = editor().schema().clone();
		let html = format!("<iframe src=\"{src}\"></iframe>");
		let read = clipboard::from_html(&schema, &html);
		let kind = read.blocks().first().map(|tree| schema.block(tree.kind).name.to_string());
		prop_assert_eq!(kind.as_deref(), expected, "{}", html);
	}
}
