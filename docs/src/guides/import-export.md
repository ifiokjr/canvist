# Import and export

In this guide you move content in and out of canvist: load JSON, HTML, Markdown, and text, paste into an existing document, export each format, and store documents safely.

## Load a document

`parse` reads any of the four formats. `fromJson` takes JSON you've already parsed, and `create` starts an empty document:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();

const fromMarkdown = engine.parse("markdown", "# Hello\n\n- one\n- two");
const fromHtml = engine.parse("html", "<h1>Hello</h1><p>It's <b>here</b>.</p>");
const fromText = engine.parse("text", "First line\nSecond line");
const fromJson = engine.fromJson({
	type: "doc",
	content: [{ type: "paragraph", content: [{ type: "text", text: "Stored" }] }],
});
const empty = engine.create();

console.log(fromHtml.export("markdown"));
// # Hello
//
// It's **here**.
```

## What happens to content canvist doesn't know

| Format   | Unknown content                                                                                |
| -------- | ---------------------------------------------------------------------------------------------- |
| JSON     | Fails with the code `schema`. Stored documents never lose data silently.                       |
| HTML     | Unknown tags keep their text as paragraphs. `<script>`, `<style>`, and attributes are dropped. |
| Markdown | Syntax for blocks you didn't register becomes plain paragraphs.                                |
| Text     | Always loads: one paragraph per line.                                                          |

That makes HTML import a safe way to clean pasted or user-supplied HTML:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse(
	"html",
	`<p onclick="steal()">Hi <b>there</b><script>alert(1)</script></p><table><tr><td>cell</td></tr></table>`,
);

console.log(doc.export("html"));
// <p>Hi <strong>there</strong></p><p>cell</p>
```

## Paste into a document

`paste` inserts content at the selection, replacing it, the way a clipboard paste does. Paste rules run, so a pasted URL becomes a link:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("markdown", "Notes: TBD");
doc.select({ path: [0], offset: 7 }, { path: [0], offset: 10 });
doc.paste("markdown", "see **the plan** at https://example.com");

console.log(doc.export("markdown"));
// Notes: see **the plan** at [https://example.com](https://example.com)
```

## Export

`export` writes any format. In TypeScript, `toJson()` returns the JSON as a typed object rather than a string:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("markdown", "## Title\n\nBody with *style*");

console.log(doc.export("html"));
// <h2>Title</h2><p>Body with <em>style</em></p>
console.log(doc.export("text"));
// Title
// Body with style
console.log(doc.toJson().content?.map((block) => block.type));
// [ "heading", "paragraph" ]
```

## Store documents as JSON

Store JSON, not HTML or Markdown. JSON is the only format that keeps every attribute and the stable block ids that comments and collaborators point at. It's also ProseMirror JSON, so TipTap reads it.

To save the selection too, for "continue where you left off", take a snapshot:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("markdown", "Draft");
doc.select({ path: [0], offset: 5 });

const saved: string = doc.snapshot(); // content and selection, as a JSON string
const restored = engine.restore(saved);
restored.insertText(" two");

console.log(restored.export("markdown"));
// Draft two
```

## In Rust and Dart

The same calls exist everywhere:

| TypeScript                 | Rust                             | Dart                           |
| -------------------------- | -------------------------------- | ------------------------------ |
| `engine.parse("html", s)`  | `engine.parse(Format::Html, s)?` | `engine.parse(Format.html, s)` |
| `engine.fromJson(json)`    | `engine.from_json(&json)?`       | `engine.fromJson(json)`        |
| `doc.export("markdown")`   | `doc.export(Format::Markdown)`   | `doc.export(Format.markdown)`  |
| `doc.paste("html", s)`     | `doc.paste(Format::Html, s)?`    | `doc.paste(Format.html, s)`    |
| `doc.snapshot()`           | `doc.snapshot()`                 | `doc.snapshot()`               |
| `engine.restore(snapshot)` | `engine.restore(&snapshot)?`     | `engine.restore(snapshot)`     |

```rust
use canvist_headless::Engine;
use canvist_headless::Format;

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let engine = Engine::starter_kit();
	let doc = engine.parse(Format::Html, "<h2>Title</h2><p>Body</p>")?;

	let json = doc.to_json();
	let copy = engine.from_json(&json)?;
	assert_eq!(copy.export(Format::Markdown), "## Title\n\nBody");
	Ok(())
}
```

## Convert without running anything

To convert formats over HTTP, without hosting canvist yourself, use [canvist Cloud Convert](../cloud/convert.md).
