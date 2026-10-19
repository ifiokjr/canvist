# Extension catalogue

Use this page to find the extensions you need: what each one does, how to add it, and the commands, queries, keys, and typing shortcuts it brings.

Every extension below ships today in Rust, TypeScript, and Dart. Names match TipTap's, so documents move between TipTap and canvist as JSON without conversion.

## How to add an extension

| Language   | Add an extension                        | Add the whole StarterKit |
| ---------- | --------------------------------------- | ------------------------ |
| TypeScript | `createEngine([bold, heading])`         | `createEngine()`         |
| Rust       | `Engine::new(\|b\| b.add(Bold))`        | `Engine::starter_kit()`  |
| Dart       | `EngineBuilder()..add(Extensions.bold)` | `Engine.starterKit()`    |

In TypeScript, import each extension by name from `@canvist/headless`. In Rust, import it from `canvist_extensions` (re-exported as `canvist_headless::canvist_extensions`). `Mod` means Cmd on Apple platforms and Ctrl elsewhere.

In the **Typing** columns, block shortcuts such as `##`, `-`, and `[ ]` fire when you type a space after them at the start of a line. Mark shortcuts such as `**text**` fire when you type the closing characters.

## Always included

Every TypeScript and Dart engine has these, so you never add them yourself. In Rust, `Engine::new` registers exactly what you add: add `Document`, `Paragraph`, and `BaseKeymap` yourself, or start from `StarterKit::add_all`, which includes them.

- **Document and paragraph.** The root, and plain paragraphs. Command `setParagraph`, query `isParagraph`, key `Mod-Alt-0`.
- **Standard keys.** Typing, Enter, Backspace, Delete, `Mod-a`, the arrow keys with Shift, Alt, and Mod, and the macOS Emacs keys (`Ctrl-a`, `Ctrl-e`, `Ctrl-k`, …).
- **Undo an input rule.** Backspace right after a typing shortcut fires puts back what you typed. Type `##`, press Backspace, and you have `##` again instead of a heading.

## Text formatting

### Bold

Bold text. Adds the `bold` mark.

| Commands                             | Queries  | Keys    | Typing                 |
| ------------------------------------ | -------- | ------- | ---------------------- |
| `toggleBold`, `setBold`, `unsetBold` | `isBold` | `Mod-b` | `**text**`, `__text__` |

```ts
import { bold, createEngine } from "@canvist/headless";

const doc = (await createEngine([bold])).parse("text", "Ship it");
doc.selectAll();
doc.commands.toggleBold();
console.log(doc.export("html"));
// <p><strong>Ship it</strong></p>
```

### Italic

Italic text. Adds the `italic` mark.

| Commands                                   | Queries    | Keys    | Typing             |
| ------------------------------------------ | ---------- | ------- | ------------------ |
| `toggleItalic`, `setItalic`, `unsetItalic` | `isItalic` | `Mod-i` | `*text*`, `_text_` |

```ts
import { createEngine, italic } from "@canvist/headless";

const doc = (await createEngine([italic])).create();
doc.typeText("A *quiet* word");
console.log(doc.export("html"));
// <p>A <em>quiet</em> word</p>
```

### Underline

Underlined text. Adds the `underline` mark. Markdown has no underline, so Markdown export drops it; HTML and JSON keep it.

| Commands                                            | Queries       | Keys    |
| --------------------------------------------------- | ------------- | ------- |
| `toggleUnderline`, `setUnderline`, `unsetUnderline` | `isUnderline` | `Mod-u` |

```ts
import { createEngine, underline } from "@canvist/headless";

const doc = (await createEngine([underline])).parse("text", "Sign here");
doc.selectAll();
doc.commands.setUnderline();
console.log(doc.export("html"));
// <p><u>Sign here</u></p>
```

### Strike

Struck-through text. Adds the `strike` mark.

| Commands                                   | Queries    | Keys          | Typing     |
| ------------------------------------------ | ---------- | ------------- | ---------- |
| `toggleStrike`, `setStrike`, `unsetStrike` | `isStrike` | `Mod-Shift-s` | `~~text~~` |

```ts
import { createEngine, strike } from "@canvist/headless";

const doc = (await createEngine([strike])).parse("markdown", "~~old~~ new");
console.log(doc.export("html"));
// <p><s>old</s> new</p>
```

### Code

Inline code. Adds the `code` mark. Code excludes the other marks: text can't be bold and code at once.

| Commands                             | Queries  | Keys    | Typing       |
| ------------------------------------ | -------- | ------- | ------------ |
| `toggleCode`, `setCode`, `unsetCode` | `isCode` | `Mod-e` | `` `text` `` |

```ts
import { code, createEngine } from "@canvist/headless";

const doc = (await createEngine([code])).create();
doc.typeText("Run `deno test` first");
console.log(doc.export("markdown"));
// Run `deno test` first
```

### Link

Links with an `href`. Adds the `link` mark. URLs become links when you type a space after them and when you paste them. Typing at the edge of a link doesn't extend it.

| Commands                         | Queries    | Typing                    |
| -------------------------------- | ---------- | ------------------------- |
| `setLink({ href })`, `unsetLink` | `linkHref` | A URL followed by a space |

```ts
import { createEngine, link } from "@canvist/headless";

const doc = (await createEngine([link])).parse("text", "Read the guide");
doc.select({ path: [0], offset: 9 }, { path: [0], offset: 14 });
doc.commands.setLink({ href: "https://example.com" });
console.log(doc.export("markdown"));
// Read the [guide](https://example.com)
```

In Rust, `Link { autolink: false, link_on_paste: false }` turns off automatic linking.

## Blocks

### Heading

Headings, levels 1 to 6. Adds the `heading` block with a `level` attribute.

| Commands                                            | Queries        | Keys                       | Typing          |
| --------------------------------------------------- | -------------- | -------------------------- | --------------- |
| `toggleHeading({ level })`, `setHeading({ level })` | `headingLevel` | `Mod-Alt-1` to `Mod-Alt-6` | `#` to `######` |

```ts
import { createEngine, heading } from "@canvist/headless";

const doc = (await createEngine([heading])).parse("text", "Overview");
doc.selectAll();
doc.commands.setHeading({ level: 2 });
console.log(doc.export("markdown"));
// ## Overview
```

In Rust, `Heading { max_level: HeadingLevel::H3 }` offers only levels 1 to 3.

### Blockquote

Quotes that wrap other blocks. Adds the `blockquote` block.

| Commands           | Queries        | Keys          | Typing |
| ------------------ | -------------- | ------------- | ------ |
| `toggleBlockquote` | `isBlockquote` | `Mod-Shift-b` | `>`    |

```ts
import { blockquote, createEngine } from "@canvist/headless";

const doc = (await createEngine([blockquote])).parse(
	"text",
	"Less, but better.",
);
doc.selectAll();
doc.commands.toggleBlockquote();
console.log(doc.export("html"));
// <blockquote><p>Less, but better.</p></blockquote>
```

### Code block

Blocks of code with an optional language. Adds the `codeBlock` block with a `language` attribute. Enter inserts a new line inside the block; Enter on an empty last line leaves it.

| Commands                                                      | Queries                            | Keys        | Typing                                |
| ------------------------------------------------------------- | ---------------------------------- | ----------- | ------------------------------------- |
| `toggleCodeBlock({ language })`, `setCodeBlock({ language })` | `isCodeBlock`, `codeBlockLanguage` | `Mod-Alt-c` | `` ``` `` or `` ```rust `` then space |

```ts
import { codeBlock, createEngine } from "@canvist/headless";

const doc = (await createEngine([codeBlock])).parse("text", "let x = 1;");
doc.selectAll();
doc.commands.setCodeBlock({ language: "rust" });
console.log(doc.query.codeBlockLanguage());
// rust
```

### Horizontal rule

A dividing line between blocks. Adds the `horizontalRule` block.

| Commands            | Typing                 |
| ------------------- | ---------------------- |
| `setHorizontalRule` | `---`, `___`, or `***` |

```ts
import { createEngine, horizontalRule } from "@canvist/headless";

const doc = (await createEngine([horizontalRule])).parse(
	"markdown",
	"Above\n\n---\n\nBelow",
);
console.log(doc.export("html"));
// <p>Above</p><hr><p>Below</p>
```

### Hard break

A line break inside a paragraph. Adds the `hardBreak` embed.

| Commands       | Keys                       |
| -------------- | -------------------------- |
| `setHardBreak` | `Shift-Enter`, `Mod-Enter` |

```ts
import { createEngine, hardBreak } from "@canvist/headless";

const doc = (await createEngine([hardBreak])).parse("text", "Line one");
doc.select({ path: [0], offset: 8 });
doc.commands.setHardBreak();
doc.insertText("Line two");
console.log(doc.export("html"));
// <p>Line one<br>Line two</p>
```

## Lists

Lists need list items. Adding `bulletList` or `orderedList` adds `listItem` for you.

### Bullet list

| Commands           | Queries        | Keys          | Typing           |
| ------------------ | -------------- | ------------- | ---------------- |
| `toggleBulletList` | `isBulletList` | `Mod-Shift-8` | `-`, `*`, or `+` |

### Ordered list

Numbered lists with a `start` number.

| Commands                                              | Queries                             | Keys          | Typing |
| ----------------------------------------------------- | ----------------------------------- | ------------- | ------ |
| `toggleOrderedList`, `setOrderedListStart({ start })` | `isOrderedList`, `orderedListStart` | `Mod-Shift-7` | `1.`   |

### List item

The items inside bullet and ordered lists. Enter splits an item, Tab nests it under the item before, and Shift-Tab lifts it out.

| Commands                                        | Keys                                     |
| ----------------------------------------------- | ---------------------------------------- |
| `splitListItem`, `sinkListItem`, `liftListItem` | `Enter`, `Tab`, `Shift-Tab`, `Backspace` |

```ts
import { bulletList, createEngine, orderedList } from "@canvist/headless";

const doc = (await createEngine([bulletList, orderedList])).create();
doc.typeText("- milk");
doc.press("Enter");
doc.typeText("eggs");
doc.press("Tab");
console.log(doc.export("markdown"));
// - milk
//   - eggs
```

### Task list

Checklists. Adds the `taskList` block and `taskItem` blocks with a `checked` attribute. It brings its own items, so it doesn't need `listItem`.

| Commands                                                                                                       | Queries                       | Keys          | Typing         |
| -------------------------------------------------------------------------------------------------------------- | ----------------------------- | ------------- | -------------- |
| `toggleTaskList`, `toggleTask`, `setTaskChecked({ checked })`, `splitTaskItem`, `sinkTaskItem`, `liftTaskItem` | `isTaskList`, `isTaskChecked` | `Mod-Shift-9` | `[ ]` or `[x]` |

```ts
import { createEngine, taskList } from "@canvist/headless";

const doc = (await createEngine([taskList])).parse(
	"markdown",
	"- [ ] Write docs",
);
doc.select({ path: [0, 0, 0], offset: 0 });
doc.commands.toggleTask();
console.log(doc.export("markdown"));
// - [x] Write docs
```

## Editing behaviour

### History

Undo and redo. Without it, `undo()` and `redo()` still work from code; History adds the commands and keys.

| Commands       | Keys                            |
| -------------- | ------------------------------- |
| `undo`, `redo` | `Mod-z`, `Mod-Shift-z`, `Mod-y` |

### Trailing node

Keeps an empty paragraph at the end of the document, so there's always somewhere to type below a list, quote, or code block. It has no commands.

## Coming soon

These extensions are planned, in roughly this order. Each will follow the same pattern: typed commands, queries, and TipTap-compatible JSON.

- **Tables:** merged cells, column resizing, and cell selection.
- **Images,** with resizing, plus video, audio, and file attachments.
- **Mentions, emoji, and slash commands,** on one shared suggestion system.
- **Text style:** highlight, colour, font family and size, subscript, superscript, and text alignment.
- **Editor helpers:** placeholder, character count, find and replace, table of contents, and drag handles.
- **Comments, version history, and tracked changes.**

To see where each one stands, watch [the releases](https://github.com/ifiokjr/canvist/releases).
