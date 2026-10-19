//! A scripted transform: turn every paragraph that starts with `TODO:` into
//! an unchecked task, and bold every `FIXME`. Reads Markdown on standard
//! input and writes Markdown.
//!
//! ```sh
//! printf 'Notes\n\nTODO: ship it\n\nFIXME later' | cargo run -p canvist_headless --example transform
//! ```

use std::io::Read;

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use canvist_headless::canvist_extensions::StarterKitHandles;

const TODO: &str = "TODO: ";
const FIXME: &str = "FIXME";

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut markdown = String::new();
	std::io::stdin().read_to_string(&mut markdown)?;

	let engine = Engine::starter_kit();
	let mut doc = engine.parse(Format::Markdown, &markdown)?;
	transform(&mut doc, engine.handles())?;

	println!("{}", doc.export(Format::Markdown));
	Ok(())
}

fn transform(doc: &mut Doc, kit: StarterKitHandles) -> Result<(), HeadlessError> {
	// Each change keeps one top-level block per paragraph, so the indexes
	// collected up front stay valid.
	for (index, text) in top_level_texts(doc) {
		for (start, _) in text.match_indices(FIXME) {
			let from = utf16_len(&text[..start]);
			let to = from + utf16_len(FIXME);
			doc.select_range(&Position::new([index], from), &Position::new([index], to))?;
			doc.run(&kit.bold.set())?;
		}

		if text.starts_with(TODO) {
			let prefix = Position::new([index], utf16_len(TODO));
			doc.select_range(&Position::new([index], 0), &prefix)?;
			doc.delete_selection()?;
			doc.run(&kit.task_list.toggle())?;
		}
	}

	Ok(())
}

/// The index and text of every top-level text block.
fn top_level_texts(doc: &Doc) -> Vec<(u32, String)> {
	let model = doc.document();
	model
		.children(None)
		.iter()
		.enumerate()
		.filter_map(|(index, &id)| Some((index as u32, model.inline(id)?.as_str().to_owned())))
		.collect()
}

fn utf16_len(text: &str) -> u32 {
	text.encode_utf16().count() as u32
}
