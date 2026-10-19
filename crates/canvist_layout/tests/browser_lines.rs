//! Server rendering wraps where the browser does, given the same font file.
//!
//! `tests/fixtures/browser-lines.json` holds where Chromium broke the lines
//! of plain paragraphs set in Inter, recorded by `@canvist/web`'s parity
//! tests (`tests/browser/ssr-lines.spec.ts`, with `CANVIST_UPDATE_GOLDENS`).
//! Here `parley` lays the same text out with the same Inter file, the same
//! theme, and Blink's line-break tailoring, and must break every line at
//! the same character.

#![cfg(feature = "parley")]

mod support;

use std::ops::Range;
use std::sync::Arc;

use canvist_layout::DocumentLayout;
use canvist_layout::LayoutOptions;
use canvist_layout::ParleyMeasurer;
use canvist_layout::StarterKitOptions;
use canvist_layout::Theme;
use serde_json::Value as Json;

/// The variable Inter the browser loaded, from `@canvist/web`'s test fonts.
const INTER: &[u8] = include_bytes!("../../../packages/canvist-web/tests/fonts/inter-variable.ttf");
const GOLDEN: &str = include_str!("fixtures/browser-lines.json");

/// A byte offset into `text` from a UTF-16 offset, as the browser counts.
fn byte_offset(text: &str, utf16: u64) -> u32 {
	let mut units = 0;

	for (offset, character) in text.char_indices() {
		if units >= utf16 {
			return offset as u32;
		}

		units += character.len_utf16() as u64;
	}

	text.len() as u32
}

/// The browser's lines of `text`, as byte ranges.
fn browser_lines(text: &str, lines: &Json) -> Vec<Range<u32>> {
	lines
		.as_array()
		.into_iter()
		.flatten()
		.map(|line| {
			let start = line[0].as_u64().unwrap_or_default();
			let end = line[1].as_u64().unwrap_or_default();
			byte_offset(text, start)..byte_offset(text, end)
		})
		.collect()
}

#[test]
fn parley_breaks_lines_where_chromium_does() {
	let golden: Json = serde_json::from_str(GOLDEN).expect("valid golden JSON");
	let cases = golden["cases"].as_array().expect("a list of cases");
	assert!(!cases.is_empty());

	for case in cases {
		let theme = Theme::starter_kit_with(&StarterKitOptions {
			body_family: Arc::from(case["bodyFamily"].as_str().expect("a family")),
			size: case["size"].as_f64().expect("a size") as f32,
			letter_spacing: case["letterSpacing"].as_f64().unwrap_or_default() as f32,
			..StarterKitOptions::default()
		});
		let texts: Vec<&str> = case["texts"]
			.as_array()
			.expect("texts")
			.iter()
			.map(|entry| entry["text"].as_str().expect("text"))
			.collect();
		let doc = support::paragraphs(&texts);
		let width = case["width"].as_f64().expect("a width") as f32;
		let options = LayoutOptions::new(width).break_override(canvist_layout::chromium_breaks);
		let mut measurer = ParleyMeasurer::new();
		measurer.register_font(INTER.to_vec());
		let mut layout = DocumentLayout::new(&doc, Arc::new(theme), options);
		layout
			.layout_all(&doc, &mut measurer)
			.expect("parley honours the contract");

		for (index, entry) in case["texts"].as_array().expect("texts").iter().enumerate() {
			let text = texts[index];
			let block = doc.children(None)[index];
			let expected = browser_lines(text, &entry["lines"]);
			let actual = layout.line_ranges(block).expect("a measured paragraph");

			assert_eq!(
				actual, expected,
				"{} at {width}px: parley and Chromium break {text:?} differently",
				case["fixture"]
			);
		}
	}
}
