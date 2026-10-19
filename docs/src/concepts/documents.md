# Documents and blocks

Read this page to understand what a canvist document is made of, so you can predict how content loads, changes, and exports.

## A document is a tree of blocks

A document holds blocks, in order. There are three kinds:

- **Text blocks** hold one run of text: paragraphs, headings, and code blocks.
- **Container blocks** hold other blocks: lists, list items, task items, and quotes.
- **Leaf blocks** hold nothing: a horizontal rule.

A document always has at least one block. A new, empty document is one empty paragraph.

## Text carries marks and embeds

Formatting inside a text block is a **mark**: bold, italic, underline, strike, inline code, and links. A mark covers a range of characters, and ranges can overlap, so text can be bold and italic at once. Some marks carry attributes; a link carries its `href`.

An **embed** is an inline object that sits inside the text, such as a hard line break. It takes up one character.

## Extensions define what a document may contain

Every block type and mark comes from an [extension](./extensions.md). An engine with only `bold` registered knows paragraphs and bold, and nothing else. That set of types is the document's **schema**.

What happens to content the schema doesn't know depends on how it arrives:

- **JSON must fit the schema.** Loading JSON with an unknown block type fails with the error code `schema`, so bad data never gets in silently.
- **HTML, Markdown, and text always load.** Anything without a matching type becomes a paragraph or plain text, so pasted and imported content never fails.

## Documents are TipTap-compatible JSON

canvist's JSON is the same ProseMirror JSON TipTap uses, with the same type names. You can move documents between TipTap and canvist without converting them.

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("markdown", "Buy **milk**");

console.log(JSON.stringify(doc.toJson()));
// {"content":[{"attrs":{"id":"1.0"},"content":[{"text":"Buy ","type":"text"},{"marks":[{"type":"bold"}],"text":"milk","type":"text"}],"type":"paragraph"}],"type":"doc"}
```

Each block carries a stable id in `attrs.id`, the attribute TipTap's UniqueID extension uses. Ids let comments, remote carets, and collaborators keep pointing at the same block while the document changes around it.

In TypeScript, `toJson()` and `fromJson()` are typed by the engine's extensions, so JSON with a block type you didn't register is a type error as well as a runtime error.

## Formats

A document reads and writes four formats. See [Import and export](../guides/import-export.md) for the details of each.

| Format     | Name in code | Reading it                                              | Writing it                                  |
| ---------- | ------------ | ------------------------------------------------------- | ------------------------------------------- |
| JSON       | `json`       | Must fit the schema, or loading fails                   | Keeps everything, including block ids       |
| HTML       | `html`       | Never fails; unknown tags fall back to paragraphs and text | Semantic HTML: `<p>`, `<h1>`, `<strong>`, … |
| Markdown   | `markdown`   | Never fails; GitHub task lists and strikethrough included | Markdown                                  |
| Plain text | `text`       | One paragraph per line                                  | One line per text block                     |

Use JSON to store documents. It is the only format that keeps everything, including attributes that HTML and Markdown can't express.

## Next

- [Extensions](./extensions.md): choose what your documents may contain.
- [Selections and positions](./selections.md): point at places inside a document.
