//! Layout with real fonts shaped by `parley`, using the bundled Atkinson
//! Hyperlegible (SIL Open Font License, see `tests/fonts`).

#![cfg(feature = "parley")]

mod support;

use std::sync::Arc;

use canvist_layout::DocumentLayout;
use canvist_layout::LayoutOptions;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Theme;
use support::paragraphs;

const FONT: &[u8] = include_bytes!("fonts/AtkinsonHyperlegible-Regular.ttf");

fn measurer() -> ParleyMeasurer {
	let mut measurer = ParleyMeasurer::new();
	let families = measurer.register_font(FONT.to_vec());
	assert_eq!(families, ["Atkinson Hyperlegible"]);
	measurer
}

fn layout(texts: &[&str], width: f32) -> DocumentLayout {
	let doc = paragraphs(texts);
	let mut layout = DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(width),
	);
	layout
		.layout_all(&doc, &mut measurer())
		.expect("parley honours the contract");
	layout
}

#[test]
fn shapes_proportional_text() {
	let narrow = layout(&["iiiiiiiiii"], 1_000.0);
	let wide = layout(&["WWWWWWWWWW"], 1_000.0);
	let width = |layout: &DocumentLayout| {
		let dump = layout.dump();
		dump.lines()
			.find_map(|line| line.trim().strip_prefix("line 0..10 y=0 h=24 base="))
			.and_then(|rest| rest.split(" w=").nth(1))
			.and_then(|w| w.parse::<f32>().ok())
			.expect("one line")
	};

	assert!(
		width(&narrow) * 2.0 < width(&wide),
		"{} vs {}",
		width(&narrow),
		width(&wide)
	);
}

#[test]
fn wraps_with_real_metrics() {
	insta::assert_snapshot!(
		layout(
			&["The quick brown fox jumps over the lazy dog, twice over."],
			180.0
		)
		.dump()
	);
}

#[test]
fn is_deterministic() {
	let text = ["Typing in the middle of a long paragraph relays out only that paragraph."];

	assert_eq!(layout(&text, 200.0).dump(), layout(&text, 200.0).dump());
}
