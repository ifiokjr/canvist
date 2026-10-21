# Quick start: Rust

In this guide you load a document, edit it with typed commands, and export it, all from Rust. It takes about five minutes. The same engine powers every other canvist platform, so what you learn here carries over.

**Status: available.** The `canvist` crate works today. It goes to crates.io with the first release; until then, install it from GitHub.

## 1. Install

You need Rust 1.95 or newer.

```sh
cargo add canvist --git https://github.com/canvist/canvist
```

`canvist` is the one crate you need: the engine's other crates are its modules, such as `canvist::headless` and `canvist::extensions`. The default build includes the headless engine and the StarterKit (paragraphs, headings, lists, tasks, quotes, code blocks, bold, italic, underline, strike, code, links, and undo). Optional features add more:

| Feature  | Adds                                                                  |
| -------- | --------------------------------------------------------------------- |
| `collab` | Yjs collaboration: join, merge updates, and serve y-websocket clients |
| `ssr`    | Server-side rendering to HTML and SVG                                 |

```sh
cargo add canvist --git https://github.com/canvist/canvist --features collab,ssr
```

## 2. Load, edit, and export a document

Replace `src/main.rs` with this:

```rust
use canvist::headless::Engine;
use canvist::headless::Format;
use canvist::headless::Position;

fn main() -> Result<(), canvist::headless::HeadlessError> {
	// An engine holds your extensions. Build one and share it.
	let engine = Engine::starter_kit();
	let kit = engine.handles();

	// A document is one piece of content you can edit.
	let mut doc = engine.parse(Format::Markdown, "# Plan\n\nShip the docs")?;

	// Select "docs" (block 1, characters 9 to 13) and make it bold.
	doc.select_range(&Position::new([1], 9), &Position::new([1], 13))?;
	doc.run(&kit.bold.toggle())?;

	assert_eq!(
		doc.export(Format::Html),
		"<h1>Plan</h1><p>Ship the <strong>docs</strong></p>"
	);
	assert_eq!(doc.export(Format::Markdown), "# Plan\n\nShip the **docs**");
	Ok(())
}
```

Run it:

```sh
cargo run
```

## 3. Pick only the extensions you need

`Engine::starter_kit()` registers everything in the StarterKit. To build a smaller editor, register extensions one by one with `Engine::new`. It returns a typed handle for each extension you add:

```rust
use canvist::extensions::Bold;
use canvist::extensions::Document;
use canvist::extensions::Paragraph;
use canvist::headless::Engine;
use canvist::headless::Format;

fn main() -> Result<(), canvist::headless::HeadlessError> {
	let engine = Engine::new(|builder| {
		builder.add(Document);
		builder.add(Paragraph);
		builder.add(Bold)
	})?;
	let bold = engine.handles();

	let mut doc = engine.parse(Format::Text, "Hello")?;
	doc.select_all()?;
	doc.run(&bold.toggle())?;

	assert_eq!(doc.export(Format::Markdown), "**Hello**");
	Ok(())
}
```

This engine has no `italic` handle, so code that tries to make text italic can't compile. That's the point: the compiler checks your editor's features for you.

## Next steps

- [Import and export](../guides/import-export.md) every format canvist reads and writes.
- [Server-side transforms](../guides/server-transforms.md): script edits over many documents.
- [Collaboration](../guides/collaboration.md): merge Yjs updates and serve y-websocket clients.
- [Server-side rendering](./ssr.md): render HTML and SVG that match the editor.
