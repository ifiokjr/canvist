//! Smoke tests for the canvist core library.
//!
//! These tests verify the core document model works correctly without needing
//! a browser. Playwright browser tests will be added as the WASM rendering
//! pipeline matures.

use canvist_core::Document;
use canvist_core::Position;
use canvist_core::Selection;
use canvist_core::Style;

#[test]
fn create_document_and_insert() {
	let mut doc = Document::new();
	doc.insert_text(Position::zero(), "Hello, canvist!");

	assert_eq!(doc.plain_text(), "Hello, canvist!");
	assert_eq!(doc.char_count(), 15);
}

#[test]
fn style_and_select_all() {
	let mut doc = Document::new();
	doc.insert_text(Position::zero(), "Bold text");

	let style = Style::new().bold();
	let sel = Selection::all(&doc);
	doc.apply_style(sel, &style);

	assert_eq!(doc.plain_text(), "Bold text");
}

#[test]
fn delete_selection() {
	let mut doc = Document::new();
	doc.insert_text(Position::zero(), "Hello, world!");

	let sel = Selection::range(Position::new(5), Position::new(7));
	doc.delete(&sel);

	assert_eq!(doc.plain_text(), "Helloworld!");
}

#[test]
fn json_roundtrip() {
	let mut doc = Document::new();
	doc.set_title("Test");
	doc.insert_text(Position::zero(), "Content");

	let json = doc.to_json().unwrap();
	let restored = Document::from_json(&json).unwrap();

	assert_eq!(restored.title(), Some("Test"));
	assert_eq!(restored.plain_text(), "Content");
}
