# Extension catalogue

Use this page to find the extensions you need: what each one does, how to add it, and the commands, queries, keys, and typing shortcuts it brings.

Every extension below ships today in Rust, TypeScript, and Dart. Names match TipTap's, so documents move between TipTap and canvist as JSON without conversion. To compare canvist with TipTap and Remirror extension by extension, see [parity](./parity.md).

## How to add an extension

| Language   | Add an extension                        | Add the whole StarterKit |
| ---------- | --------------------------------------- | ------------------------ |
| TypeScript | `createEngine([bold, heading])`         | `createEngine()`         |
| Rust       | `Engine::new(\|b\| b.add(Bold))`        | `Engine::starter_kit()`  |
| Dart       | `EngineBuilder()..add(Extensions.bold)` | `Engine.starterKit()`    |

In TypeScript, import each StarterKit extension by name from `@canvist/headless`. In Rust, import it from `canvist_extensions` (re-exported as `canvist_headless::canvist_extensions`). `Mod` means Cmd on Apple platforms and Ctrl elsewhere.

Extensions beyond the StarterKit, from [subscript](#subscript-and-superscript) on, each sit behind a Cargo feature of their own, and `full` turns them all on. In TypeScript they live in a second WebAssembly module, `@canvist/headless/full`, so the StarterKit module stays small: import `createEngine` and every extension from there instead, with the same API. The full module loads the emoji table the first time an engine registers `emoji`. The Dart package includes every extension. The browser editor, `@canvist/web`, includes the StarterKit only for now, to keep its download small.

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

### Subscript and superscript

Text below or above the baseline, smaller than the text around it. Subscript adds the `subscript` mark and superscript the `superscript` mark. The two exclude each other. Markdown has no syntax for them, so Markdown export writes the `<sub>` and `<sup>` tags GitHub renders, and Markdown import reads them back.

| Commands                                                  | Queries         | Keys    |
| --------------------------------------------------------- | --------------- | ------- |
| `toggleSubscript`, `setSubscript`, `unsetSubscript`       | `isSubscript`   | `Mod-,` |
| `toggleSuperscript`, `setSuperscript`, `unsetSuperscript` | `isSuperscript` | `Mod-.` |

```ts
import { createEngine, subscript, superscript } from "@canvist/headless/full";

const doc = (await createEngine([subscript, superscript])).parse(
	"text",
	"H2O and E=mc2",
);
doc.select({ path: [0], offset: 1 }, { path: [0], offset: 2 });
doc.commands.toggleSubscript();
doc.select({ path: [0], offset: 12 }, { path: [0], offset: 13 });
doc.commands.toggleSuperscript();
console.log(doc.export("html"));
// <p>H<sub>2</sub>O and E=mc<sup>2</sup></p>
```

### Highlight

Text marked like a highlighter pen. Adds the `highlight` mark with a `color` attribute; a highlight without a colour uses the theme's. In Rust, `Highlight { multicolor: false }` drops the attribute so every highlight uses the theme's colour, as TipTap does by default.

| Commands                                                                    | Queries                         | Keys          | Typing     |
| --------------------------------------------------------------------------- | ------------------------------- | ------------- | ---------- |
| `toggleHighlight({ color? })`, `setHighlight({ color? })`, `unsetHighlight` | `isHighlight`, `highlightColor` | `Mod-Shift-h` | `==text==` |

`toggleHighlight` with a colour recolours text highlighted in another colour and removes a highlight in the same colour.

```ts
import { createEngine, highlight } from "@canvist/headless/full";

const doc = (await createEngine([highlight])).create();
doc.typeText("Read ==this== first");
console.log(doc.export("html"));
// <p>Read <mark>this</mark> first</p>
doc.selectAll();
doc.commands.setHighlight({ color: "#fef08a" });
console.log(doc.query.highlightColor());
// #fef08a
```

### Text style

Inline styles, as in TipTap's TextStyleKit. Text style adds the `textStyle` mark, a `<span style>`, and the other five extensions each add one attribute to it, so text can have a colour and a size at once. Clearing the last attribute removes the mark. Register text style with at least one of them. Markdown has no styles, so Markdown export writes the text plain.

| Extension         | Attribute         | Commands                                                          | Query             |
| ----------------- | ----------------- | ----------------------------------------------------------------- | ----------------- |
| `textStyle`       | none              | `removeEmptyTextStyle`                                            |                   |
| `color`           | `color`           | `setColor({ color })`, `unsetColor`                               | `color`           |
| `backgroundColor` | `backgroundColor` | `setBackgroundColor({ backgroundColor })`, `unsetBackgroundColor` | `backgroundColor` |
| `fontFamily`      | `fontFamily`      | `setFontFamily({ fontFamily })`, `unsetFontFamily`                | `fontFamily`      |
| `fontSize`        | `fontSize`        | `setFontSize({ fontSize })`, `unsetFontSize`                      | `fontSize`        |
| `lineHeight`      | `lineHeight`      | `setLineHeight({ lineHeight })`, `unsetLineHeight`                | `lineHeight`      |

Values are CSS strings such as `#e8590c`, `Georgia, serif`, `20px`, and `1.5`. Each query returns the value every selected character shares, or `null`. The canvas draws `#rrggbb` colours, pixel sizes, and unitless line heights, and keeps other values in the document as they are.

```ts
import {
	color,
	createEngine,
	fontSize,
	textStyle,
} from "@canvist/headless/full";

const doc = (await createEngine([textStyle, color, fontSize])).parse(
	"text",
	"Warm",
);
doc.selectAll();
doc.commands.setColor({ color: "#e8590c" });
doc.commands.setFontSize({ fontSize: "20px" });
console.log(doc.export("html"));
// <p><span style="color: #e8590c; font-size: 20px">Warm</span></p>
doc.commands.unsetColor();
console.log(doc.query.fontSize());
// 20px
```

### Text alignment

Left, centred, right, and justified text. Text alignment adds a `textAlign` attribute to paragraphs and headings (in Rust, `TextAlign { types }` picks other text blocks). The canvas aligns the lines, and justified lines widen their spaces, except the last line of a paragraph. HTML writes TipTap's `style="text-align: center"` and reads it back; Markdown has no alignment, so it's dropped there. An unset alignment follows the text: left, or right for right-to-left paragraphs.

| Commands                                                                          | Queries     | Keys                                   |
| --------------------------------------------------------------------------------- | ----------- | -------------------------------------- |
| `setTextAlign({ alignment })`, `toggleTextAlign({ alignment })`, `unsetTextAlign` | `textAlign` | `Mod-Shift-l`, `e`, `r`, `j` (justify) |

`alignment` is `left`, `center`, `right`, or `justify`, and `textAlign` answers the alignment every selected block shares, or `null`.

```ts
import { createEngine, heading, textAlign } from "@canvist/headless/full";

const doc = (await createEngine([heading, textAlign])).parse(
	"markdown",
	"# Title",
);
doc.commands.setTextAlign({ alignment: "center" });
console.log(doc.export("html"));
// <h1 style="text-align: center">Title</h1>
```

## Mentions, hashtags, and emoji

Mentions, hashtags, and emoji share one suggestion system, like TipTap's Suggestion utility and Remirror's suggesters. Typing the trigger character (`@`, `#`, or `:`) after a space or at the start of a line starts a suggestion. While it's active, a query reports the trigger and what you've typed since, and you show your own popup at the caret with the matches. Picking one runs a command that replaces the trigger and query, then adds a space. Escape, or the dismiss command, hides the suggestion until the trigger is typed again. Suggestions never start in code blocks.

### Mention

People, teams, or anything else with an id. It adds TipTap's `mention` embed with `id`, `label`, and `mentionSuggestionChar` attributes, drawn as `@label`. Backspace straight after a mention turns it back into `@`, so the suggestion opens again. HTML uses TipTap's `<span data-type="mention">`; Markdown and plain text write `@label`.

| Commands                                                    | Queries             | Keys                  |
| ----------------------------------------------------------- | ------------------- | --------------------- |
| `insertMention({ id, label? })`, `dismissMentionSuggestion` | `mentionSuggestion` | `Escape`, `Backspace` |

`mentionSuggestion` answers `{ char, query }` while a mention is suggested, or `null`. In Rust, `Mention { char: '+', allow_spaces: true }` changes the trigger and allows `@Ada Lovelace`.

```ts
import { createEngine, mention } from "@canvist/headless/full";

const doc = (await createEngine([mention])).create();
doc.typeText("Thanks @ad");
console.log(JSON.stringify(doc.query.mentionSuggestion()));
// {"char":"@","query":"ad"}
doc.commands.insertMention({ id: "7", label: "Ada" });
doc.typeText("!");
console.log(doc.export("text"));
// Thanks @Ada !
```

### Hashtag

Tags such as `#rust`, marked as you type. Neither TipTap nor Remirror has one. It adds a `hashtag` mark with the tag (without `#`) in its `tag` attribute. A tag is marked when you type the space or punctuation after it, and when you paste it. `#` at the start of a line still makes a heading.

| Commands                                             | Queries                         | Keys     |
| ---------------------------------------------------- | ------------------------------- | -------- |
| `insertHashtag({ tag })`, `dismissHashtagSuggestion` | `hashtagSuggestion`, `hashtags` | `Escape` |

`hashtags` lists every tag in the document, sorted, for a tag cloud or a filter.

```ts
import { createEngine, hashtag } from "@canvist/headless/full";

const doc = (await createEngine([hashtag])).create();
doc.typeText("Loving #rust and #wasm today");
console.log(doc.query.hashtags().join(", "));
// rust, wasm
```

### Emoji

GitHub's 1,870 emoji and their shortcodes. Typing a known `:shortcode:` inserts the emoji. It adds TipTap's `emoji` embed, with the shortcode in `name` and, in canvist, the emoji itself in `emoji`, so every platform draws it without looking it up. A TipTap document without `emoji` shows `:name:`. HTML uses TipTap's `<span data-type="emoji">`; Markdown and plain text write the emoji.

| Commands                                       | Queries                                             | Keys     | Typing        |
| ---------------------------------------------- | --------------------------------------------------- | -------- | ------------- |
| `setEmoji({ name })`, `dismissEmojiSuggestion` | `emojiSuggestion`, `searchEmoji({ query, limit? })` | `Escape` | `:shortcode:` |

`searchEmoji` returns up to `limit` (10 by default) `{ name, emoji }` matches, shortcodes starting with the query first.

```ts
import { createEngine, emoji } from "@canvist/headless/full";

const doc = (await createEngine([emoji])).create();
doc.typeText("Shipped :rocket: :tad");
console.log(doc.query.searchEmoji({ query: "tad", limit: 1 })[0].emoji);
// 🎉
doc.commands.setEmoji({ name: "tada" });
console.log(doc.export("text"));
// Shipped 🚀 🎉
```

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

### Media

Images, YouTube and Twitch videos, audio, and file attachments, each an atomic block whose attributes say what to show. The canvas reserves a slot for it, sized by its `height` attribute, and your app draws the picture, player, or file card there, the way a TipTap node view would. Each command inserts its block at the caret, as a horizontal rule is inserted.

| Extension | Block     | Command                                                       | Attributes                                          |
| --------- | --------- | ------------------------------------------------------------- | --------------------------------------------------- |
| `image`   | `image`   | `setImage({ src, alt?, title?, width?, height? })`            | `src`, `alt`, `title`, `width`, `height`            |
| `youtube` | `youtube` | `setYoutubeVideo({ src, start?, width?, height? })`           | `src`, `start` (0), `width` (640), `height` (480)   |
| `twitch`  | `twitch`  | `setTwitchVideo({ src, width?, height?, autoplay?, muted? })` | `src`, `width`, `height`, `autoplay`, `muted`       |
| `audio`   | `audio`   | `setAudio({ src, controls?, autoplay?, loop?, muted? })`      | `src`, `controls` (on), `autoplay`, `loop`, `muted` |
| `file`    | `file`    | `insertFile({ url, fileName?, fileType?, fileSize? })`        | `url`, `fileName`, `fileType`, `fileSize`           |

Typing `![alt](src "title")` inserts an image. HTML uses the markup TipTap writes: `<img>`, `<audio>`, and `<iframe>` inside `<div data-youtube-video>` or `<div data-twitch-video>`; files use `<div data-type="file">`. Markdown writes images as `![alt](src "title")` and reads them back; other media become links.

```ts
import { createEngine, image } from "@canvist/headless/full";

const doc = (await createEngine([image])).create();
doc.commands.setImage({ src: "cat.png", alt: "A cat" });
console.log(doc.export("markdown"));
// ![A cat](cat.png)
```

### Details

A summary that opens and closes the content below it, like HTML's `<details>`. Adds the `details` block, with an `open` attribute, holding a `detailsSummary` text block and a `detailsContent` block. The canvas draws a closed one as its summary beside a `▸` and lays out none of its content.

| Commands                                          | Queries       | Keys                                     |
| ------------------------------------------------- | ------------- | ---------------------------------------- |
| `setDetails`, `unsetDetails`, `toggleDetailsOpen` | `detailsOpen` | `Enter` in the summary opens the content |

`setDetails` wraps the selected blocks and puts the caret in the empty summary. HTML uses TipTap's markup, `<details><summary>` with `<div data-type="detailsContent">`, and Markdown writes the `<details>` HTML GitHub renders.

```ts
import { createEngine, details } from "@canvist/headless/full";

const doc = (await createEngine([details])).create();
doc.insertText("The butler did it.");
doc.commands.setDetails();
doc.insertText("Spoiler");
doc.commands.toggleDetailsOpen();
console.log(doc.query.detailsOpen());
// false
```

### Callout

A tinted box for notes, tips, warnings, and errors. Adds the `callout` block with a `type`: `info` (the default), `warning`, `error`, `success`, or `blank`. Typing `:::warning` at the start of a line makes one.

| Commands                                               | Queries       |
| ------------------------------------------------------ | ------------- |
| `toggleCallout({ type? })`, `setCalloutType({ type })` | `calloutType` |

HTML writes `<div data-callout-type="warning">`. Markdown writes GitHub's alerts and reads them back: `info` is `[!NOTE]`, `success` is `[!TIP]`, `blank` is `[!IMPORTANT]`, `warning` is `[!WARNING]`, and `error` is `[!CAUTION]`.

```ts
import { callout, createEngine } from "@canvist/headless/full";

const doc = (await createEngine([callout])).create();
doc.insertText("Mind the gap");
doc.commands.toggleCallout({ type: "warning" });
console.log(doc.export("markdown"));
// > [!WARNING]
// > Mind the gap
```

### Mathematics

LaTeX formulas, inline as the `inlineMath` embed and on their own line as the `blockMath` block, each holding its source in `latex`. Typing `$$x^2$$` inserts an inline formula, and `$$$x^2$$$` alone on a line a block one. The canvas draws inline LaTeX as its source in a tinted box and reserves a slot for block formulas, so your app can typeset them with KaTeX or MathJax.

| Commands                                                    |
| ----------------------------------------------------------- |
| `insertInlineMath({ latex })`, `insertBlockMath({ latex })` |

HTML uses TipTap's `<span data-type="inline-math">` and `<div data-type="block-math">`, with the source in `data-latex`. Markdown writes `$x^2$` and `$$` blocks and reads them back; a price such as `$5` stays text.

```ts
import { createEngine, mathematics } from "@canvist/headless/full";

const doc = (await createEngine([mathematics])).create();
doc.insertText("Area ");
doc.commands.insertInlineMath({ latex: "\\pi r^2" });
console.log(doc.export("markdown"));
// Area $\pi r^2$
```

### Table

TipTap's TableKit: a `table` of `tableRow`s holding `tableCell`s and `tableHeader`s, each cell holding blocks. Cells take TipTap's `colspan`, `rowspan`, and `colwidth` (one pixel width per column a cell spans). The canvas lays a table out on a grid: each column as wide as a cell's `colwidth` says, or an even share of what is left; each row as tall as its tallest cell; and hairlines between cells.

| Commands                                                                                         | Queries     | Keys                                           |
| ------------------------------------------------------------------------------------------------ | ----------- | ---------------------------------------------- |
| `insertTable({ rows?, cols?, withHeaderRow? })`                                                  | `tableCell` | `Tab` / `Shift-Tab` move between cells         |
| `addColumnBefore`, `addColumnAfter`, `deleteColumn`, `addRowBefore`, `addRowAfter`, `deleteRow`  |             | `Tab` in the last cell adds a row              |
| `mergeCells`, `splitCell`, `mergeOrSplit`, `deleteTable`                                         |             | `Backspace` and `Delete` stop at a cell's edge |
| `toggleHeaderRow`, `toggleHeaderColumn`, `toggleHeaderCell`, `setCellAttribute({ name, value })` |             |                                                |
| `goToNextCell`, `goToPreviousCell`, `fixTables`                                                  |             |                                                |

A selection from one cell to another selects the rectangle of cells between them, grown to whole merged cells, and the commands act on it. `tableCell` answers where the selection head is: its row, column, spans, whether it is a header, and the table's size. A cell spanning a new row's or column's place grows instead of splitting, and rows that come out short are padded with empty cells, as TipTap's `fixTables` does.

HTML reads and writes `<table>` with TipTap's `colspan`, `rowspan`, and `colwidth`. Markdown writes a GitHub pipe table when the table fits one exactly: no merged cells, one line of text per cell, header cells in the first row only, and the same alignment down each column. Any other table is written as HTML, which GitHub renders and the reader reads back.

```ts
import { createEngine, table } from "@canvist/headless/full";

const doc = (await createEngine([table])).create();
doc.commands.insertTable({ rows: 2, cols: 2 });
doc.typeText("Name");
doc.press("Tab");
doc.typeText("Score");
doc.press("Tab");
doc.typeText("Ada");
console.log(doc.export("markdown"));
// | Name | Score |
// | --- | --- |
// | Ada |  |
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

### Typography

Smart punctuation as you type, with TipTap's rules plus Remirror's double arrow. It has no commands or keys. Backspace straight after a replacement puts back what you typed, and rules never run in code blocks.

| Typing                       | Becomes                      | Typing                       | Becomes            |
| ---------------------------- | ---------------------------- | ---------------------------- | ------------------ |
| `"quote"`, `'quote'`, `it's` | `“quote”`, `‘quote’`, `it’s` | `--`, `...`                  | `—`, `…`           |
| `->`, `<-`, `<->`            | `→`, `←`, `↔`                | `(c)`, `(tm)`, `(sm)`, `(r)` | `©`, `™`, `℠`, `®` |
| `1/2`, `1/4`, `3/4`          | `½`, `¼`, `¾`                | `+/-`, `!=`                  | `±`, `≠`           |
| `<<`, `>>`                   | `«`, `»`                     | `2x3`, `x^2`, `x^3`          | `2×3`, `x²`, `x³`  |

Fractions, `2x3`, `^2`, and `^3` change when you type the space or punctuation after them: canvist never runs rules on letters and digits, which keeps typing fast. In Rust, `Typography::default().without(&["emDash"]).with_output("laquo", "‹")` turns off or rewords a rule, by TipTap's option names.

```ts
import { createEngine, typography } from "@canvist/headless/full";

const doc = (await createEngine([typography])).create();
doc.typeText('"Wait" -- it\'s 1/2 done...');
console.log(doc.export("text"));
// “Wait” — it’s ½ done…
```

### Character count

Counts characters and words, and with a limit, refuses edits that would take the document past it. A character is what a reader sees as one, so `👍🏽` counts once; TipTap counts UTF-16 code units. Inline embeds count as none. A paste that would overflow the limit is refused whole, and deleting always works, even in a document already over the limit.

| Commands                       | Queries                                 |
| ------------------------------ | --------------------------------------- |
| `setCharacterLimit({ limit })` | `characters`, `words`, `characterLimit` |

`limit` is a number, or `null` for no limit. In Rust, `CharacterCount { limit: Some(280) }` sets the starting limit.

```ts
import { characterCount, createEngine } from "@canvist/headless/full";

const doc = (await createEngine([characterCount])).create();
doc.commands.setCharacterLimit({ limit: 5 });
doc.typeText("Hello, world");
console.log(doc.export("text"), doc.query.characters());
// Hello 5
```

### Placeholder

Hint text, such as "Write something …", in empty blocks. The placeholder isn't part of the document: the canvas draws it in the block in a faded text colour. By default only the empty block holding the caret shows it, and only at the top level, as in TipTap. In Rust, `Placeholder { text, show_only_current, include_children }` changes that, and `PlaceholderHandle::placeholders` lists the blocks to pass to `DisplayOptions::placeholder`.

| Commands                          | Queries       |
| --------------------------------- | ------------- |
| `setPlaceholder({ placeholder })` | `placeholder` |

`placeholder` answers the text the caret's block shows, or `null`.

```ts
import { createEngine, placeholder } from "@canvist/headless/full";

const doc = (await createEngine([placeholder])).create();
doc.commands.setPlaceholder({ placeholder: "Tell a story" });
console.log(doc.query.placeholder());
// Tell a story
```

## Document tools

Tools that work across the whole document rather than on one block.

### Find and replace

A search term held beside the document, outside its history, and commands that step through and replace its matches. Matches are found inside text blocks, case-insensitively unless `setCaseSensitive` turns case on. `nextSearchResult` and `previousSearchResult` select the next or previous match from the selection, wrapping at the ends. `replace` replaces the selected match and selects the next. `replaceAll` replaces every match as one undo step.

| Commands                                                                                              | Queries         |
| ----------------------------------------------------------------------------------------------------- | --------------- |
| `setSearchTerm({ term })`, `setCaseSensitive({ caseSensitive })`                                      | `searchResults` |
| `nextSearchResult`, `previousSearchResult`, `replace({ replacement })`, `replaceAll({ replacement })` |                 |

`searchResults` answers with the term, how many matches it has, and which one is selected, or `null` when nothing is being searched for. To draw every match, pass each one's range to the layout's `DisplayOptions` highlights; in Rust, `FindAndReplaceHandle::matches` lists them.

```ts
import { createEngine, findAndReplace } from "@canvist/headless/full";

const doc = (await createEngine([findAndReplace])).parse(
	"text",
	"one fish two fish",
);
doc.commands.setSearchTerm({ term: "fish" });
doc.commands.replaceAll({ replacement: "cat" });
console.log(doc.export("text"));
// one cat two cat
```

### Table of contents

The headings as an outline. `tableOfContents` lists them in order with their level, text, outline number, and whether the selection is under it. Numbers follow the outline, not the levels: a level 3 heading straight under a level 1 heading is `1.1`, as TipTap numbers them. `goToHeading({ index })` puts the caret at the start of a heading, so your app can scroll to it.

```ts
import { createEngine, heading, tableOfContents } from "@canvist/headless/full";

const doc = (await createEngine([heading, tableOfContents])).parse(
	"markdown",
	"# Intro\n\n### Detail\n\n## Part",
);
console.log(
	doc.query.tableOfContents().map((entry) => `${entry.number} ${entry.text}`),
);
// [ "1 Intro", "1.1 Detail", "1.2 Part" ]
```

### Comments

Ranges of text tied to discussion threads. Adds the `comment` mark, whose `threads` attribute lists the ids of the threads on that text; one mark holding a list lets comments overlap. The threads themselves (messages, authors, resolution) belong to your app.

| Commands                                                 | Queries                      |
| -------------------------------------------------------- | ---------------------------- |
| `setComment({ threadId })`, `unsetComment({ threadId })` | `activeComments`, `comments` |

`setComment` adds a thread to the selected text, and `unsetComment` removes it everywhere. `activeComments` lists the threads at the caret, and `comments` every thread in the document with the text it covers. The theme tints commented text; HTML writes `<span data-type="comment" data-threads="…">`, and Markdown drops the mark.

### Drag handle

The commands behind a drag handle. The handle is your app's UI: place it with the layout's `DocumentLayout::block_rect` and find where a drop lands with `hit_block`. `moveBlockUp` and `moveBlockDown` (`Alt-↑` and `Alt-↓`) swap the selected blocks with their neighbour, reaching for the enclosing block at a container's edge, so a lone paragraph in a list item moves the item. `moveBlocksTo({ index })` puts them before sibling `index`, as a drop does.

### Clear formatting

`unsetAllMarks` removes every mark from the selection, or at a caret from what is typed next. `clearNodes` turns the selected blocks into paragraphs and lifts them out of quotes, callouts, and lists. `clearFormatting` does both as one undo step, on `Mod-\`, as in Google Docs.

## Coming soon

These extensions are planned, in roughly this order. Each will follow the same pattern: typed commands, queries, and TipTap-compatible JSON.

- **Indentation, text case, and invisible characters.**
- **Columns,** on the table grid.
- **Tracked changes and version compare,** after comments.

[Parity with TipTap and Remirror](./parity.md) lists every extension in both, with what canvist ships, plans, or leaves to the host.
