# Selections and positions

Read this page to learn how to point at a place in a document, so your commands apply exactly where you mean.

## Positions

A position is a **path** to a text block and an **offset** inside its text:

- `path` walks the block tree from the root. `[0]` is the first top-level block. `[1, 0, 0]` is the first block inside the first item of the second top-level block, such as the paragraph in a list item.
- `offset` counts UTF-16 code units, the way JavaScript and Dart index strings. In Rust, use the same unit, not byte offsets.

Positions mean the same thing in Rust, TypeScript, and Dart, so a position your server stores works in every client.

```text
# Plan                    [0]
- Write the docs          [1, 0, 0]   (bulletList → listItem → paragraph)
- Ship                    [1, 1, 0]
```

## Selections

A selection has an **anchor**, where it started, and a **head**, where it ends. When they're equal, the selection is a caret. The head can come before the anchor, as it does when someone selects backwards.

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("markdown", "# Plan\n\n- Write the docs\n- Ship");

// A caret after "Write".
doc.select({ path: [1, 0, 0], offset: 5 });

// "the docs" in the first item.
doc.select({ path: [1, 0, 0], offset: 6 }, { path: [1, 0, 0], offset: 14 });
doc.commands.toggleItalic();

// Everything.
doc.selectAll();
console.log(doc.query.isItalic());
// false

console.log(doc.export("markdown"));
// # Plan
//
// - Write *the docs*
// - Ship
```

A selection can span blocks. A command then applies to every block it touches: `setHeading` turns each selected text block into a heading, and `toggleBold` bolds every selected character.

## Selections are explicit

A headless engine has no screen, so it has no idea where "the next line" is. You always set the selection yourself, by position, before you run a command. In an editor UI, the host sets it for you as the user clicks and types.

If a position doesn't exist, selecting it fails with the code `invalidPosition`, so a stale position never edits the wrong place.

## Find the text you want to change

To act on text by content, read the blocks, find the match, and convert its index to a position. In Rust, `doc.document()` gives you the model:

```rust
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Position;

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let engine = Engine::starter_kit();
	let kit = engine.handles();
	let mut doc = engine.parse(Format::Markdown, "Status: DONE\n\nNext: TODO")?;

	let model = doc.document();
	let blocks: Vec<(u32, String)> = model
		.children(None)
		.iter()
		.enumerate()
		.filter_map(|(index, &id)| Some((index as u32, model.inline(id)?.as_str().to_owned())))
		.collect();

	for (index, text) in blocks {
		if let Some(start) = text.find("TODO") {
			// Offsets count UTF-16 code units.
			let from = text[..start].encode_utf16().count() as u32;
			let to = from + "TODO".encode_utf16().count() as u32;
			doc.select_range(&Position::new([index], from), &Position::new([index], to))?;
			doc.run(&kit.bold.set())?;
		}
	}

	assert_eq!(doc.export(Format::Markdown), "Status: DONE\n\nNext: **TODO**");
	Ok(())
}
```

## Next

- [Server-side transforms](../guides/server-transforms.md): this pattern over many documents.
- [Commands and typed handles](./commands.md): what to run once the selection is set.
