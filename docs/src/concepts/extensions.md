# Extensions

Read this page to learn how extensions shape your editor, so you can register exactly the features your app needs.

## Every feature is an extension

canvist starts with almost nothing: a document, paragraphs, and the standard editing keys (typing, Backspace, Enter, arrows). Everything else is an extension you register:

- **Bold** adds the `bold` mark, `Mod-b`, `**text**` as you type, and the `toggleBold` command.
- **Heading** adds the `heading` block, `#` to `######` as you type, `Mod-Alt-1` to `Mod-Alt-6`, and the `setHeading` command.
- **Task list** adds task lists and task items, `[ ]` as you type, and commands to check and uncheck tasks.

An extension can contribute any of these:

| Part               | What it does                                        | Example                        |
| ------------------ | --------------------------------------------------- | ------------------------------ |
| Schema             | The blocks, marks, and embeds documents may contain | `heading`, `bold`, `hardBreak` |
| Commands           | Changes you can run against the selection           | `toggleBold`, `setHeading`     |
| Queries            | Questions about the selection                       | `isBold`, `headingLevel`       |
| Keyboard shortcuts | Keys that run commands                              | `Mod-b`, `Mod-Shift-8`         |
| Input rules        | Text patterns that format as you type               | `##` → heading, `-` → list     |
| Paste rules        | Formatting applied to pasted text                   | A pasted URL becomes a link    |

The [extension catalogue](../extensions/readme.md) lists every extension with its commands.

## Register only what you need

Fewer extensions means a smaller schema, fewer commands to reason about, and content that can only take the shapes you allow. A comment box doesn't need tables. A chat input doesn't need headings.

```ts
import { bold, createEngine, italic, link } from "@canvist/headless";

// A comment box: paragraphs, bold, italic, and links.
const engine = await createEngine([bold, italic, link]);
const doc = engine.parse("markdown", "## Not a heading here\n\nJust **text**");

console.log(doc.export("markdown"));
// Not a heading here
//
// Just **text**
```

The heading became a plain paragraph, because this engine has no `heading` block.

## The StarterKit

The StarterKit is TipTap's starter set in one call: paragraphs, headings, quotes, code blocks, hard breaks, horizontal rules, bullet, ordered, and task lists, bold, italic, underline, strike, inline code, links, undo and redo, and a trailing paragraph. It's the default when you don't pass a list:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine(); // the StarterKit
console.log(engine.extensions.length);
// 21
```

The count includes the four extensions every engine has: the document, paragraphs, the standard keys, and undoing an input rule with Backspace.

## In Rust, features are compiled out too

In Rust, each extension sits behind a Cargo feature of `canvist_extensions`, so extensions you don't enable aren't compiled at all. Register extensions with `Engine::new`, and each `builder.add` returns that extension's typed handle. `Engine::new` registers exactly what you add, so include `Document` and `Paragraph`, and `BaseKeymap` if you want the standard editing keys:

```rust
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::canvist_extensions::Bold;
use canvist_headless::canvist_extensions::Document;
use canvist_headless::canvist_extensions::Heading;
use canvist_headless::canvist_extensions::HeadingLevel;
use canvist_headless::canvist_extensions::Paragraph;

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let engine = Engine::new(|builder| {
		builder.add(Document);
		builder.add(Paragraph);
		(builder.add(Bold), builder.add(Heading::default()))
	})?;
	let (bold, heading) = engine.handles();

	let mut doc = engine.parse(Format::Text, "Title")?;
	doc.select_all()?;
	doc.run(&heading.set(HeadingLevel::H2))?;
	doc.run(&bold.toggle())?;

	assert_eq!(doc.export(Format::Markdown), "## **Title**");
	Ok(())
}
```

## Your own extensions

The extensions in the catalogue are built on the public `Extension` trait in `canvist_editor`, and you can implement it yourself in Rust. A custom extension can add blocks, marks, and embeds, plus commands, queries, keyboard shortcuts, and input rules. [Build your own editor](../extensions/build-your-own-editor.md#add-your-own-extension) walks through one.

Two limits today:

- **The bindings register StarterKit extensions by name.** TypeScript and Dart can't load a custom Rust extension yet.
- **Formats and rendering know the StarterKit's types.** A custom block exports as its text in HTML and Markdown, and JSON keeps it whole.

## Next

- [Commands and typed handles](./commands.md): run, check, and query what extensions add.
- [Build your own editor](../extensions/build-your-own-editor.md): grow an editor one extension at a time.
