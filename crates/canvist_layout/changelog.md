All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Add details, callouts, and math

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #175](https://github.com/canvist/canvist/pull/175)

Three block extensions, each behind its own Cargo feature and included in `full`:

- `Details` (`details`): TipTap's `details` block, holding a `detailsSummary` and a `detailsContent`, with an `open` attribute. Use `setDetails`, `unsetDetails`, `toggleDetailsOpen`, and the `detailsOpen` query. Enter in a summary opens the details and moves into the content; Backspace at the start of a summary unwraps the details, and no edit leaves a details without its summary or content. Layout draws a closed one as its summary beside a `▸` disclosure marker and lays out none of the hidden content.
- `Callout` (`callout`): Remirror's tinted box, with a `type` of `info`, `warning`, `error`, `success`, or `blank`. Use `toggleCallout`, `setCalloutType`, and the `calloutType` query. Typing `:::warning` at the start of a line makes one, and the StarterKit theme colours each type.
- `Mathematics` (`mathematics`): TipTap's `inlineMath` embed and `blockMath` block, holding LaTeX in `latex`. Use `insertInlineMath` and `insertBlockMath`, or type `$$x^2$$` and `$$$x^2$$$`. Layout draws inline LaTeX as its source and reserves a slot for block formulas, so the host can typeset them.

HTML reads and writes TipTap's markup:

- `<details>` with a `<summary>` and `<div data-type="detailsContent">`;
- `<div data-callout-type>`;
- `<span data-type="inline-math">` and `<div data-type="block-math">`, with the source in `data-latex`.

Markdown writes callouts as GitHub alerts (`> [!WARNING]`), details as GitHub's `<details>` HTML, and formulas as `$x$` and `$$` blocks, and reads all three back. A price such as `$5` stays text.

Server-rendered HTML marks a callout as a `role="note"` labelled with its type, which otherwise only its colour shows, and a block formula as `role="math"` labelled with its LaTeX.

#### Add ink drawings and kinetic type, drawn by the canvas itself

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #179](https://github.com/canvist/canvist/pull/179)

Two canvas-native extensions, each behind its own Cargo feature and included in `full`.

**`Ink` (`ink`)** adds `ink` blocks: freehand drawings in a space 1000 units wide and `height` units tall, scaled with their column.

- Strokes are `points` lists of `x, y` pairs with an optional `color` and `width`.
- Commands: `insertInk`, `addInkStroke`, `undoInkStroke`, and `clearInk`. Query: `inks`.
- Each stroke is one undo step and one Yjs update.
- HTML writes a `<figure data-type="ink" role="img" aria-label="Drawing">` with an SVG of the drawing and the strokes as JSON. Markdown writes the same HTML, and both read it back.

**`Kinetic` (`kinetic`)** adds the `kinetic` mark, whose `effect` is `wave`, `shake`, `pulse`, or `rainbow`.

- Commands: `setKinetic`, `toggleKinetic` (another effect switches to it), and `unsetKinetic`. Query: `kineticEffect`.
- HTML writes `<span data-kinetic="…">`. Markdown keeps the text.

`canvist_layout`:

- `DisplayItem::Stroke` is new, and the SVG renderer, `canvist_web`'s Canvas2D painter, and `canvist_paint` draw it with round caps and joins. A stroke holds a range of the new `DisplayList::points`, read with `DisplayList::stroke_points`, and its bounds, so no item but a text run owns memory and lists of text stay as fast to build as before.
- `LeafStyle::Ink` lays out drawings. `Motion` and `AttrBinding::Motion` set how text moves.
- `DisplayOptions::time` moves kinetic text glyph by glyph, and `DisplayList::animated` says when a later frame would differ. Without a time, text is still, as it should be for readers who prefer reduced motion.
- The HTML renderer draws ink as inline SVG in a figure named as an image, and animates kinetic text with CSS, off under `prefers-reduced-motion`.
- New default features `ink` and `kinetic` hold this code. `canvist_web` builds without them, as it does without `tables`.

`canvist_core`: `ValueType::sample` gives lists two items, so lists of pairs get valid samples.

`canvist_canvas` and `canvist_canvas_wasm` turn on `canvist_layout`'s new `ink` and `kinetic` features.

#### Add find and replace, a table of contents, comments, block moves, and clear formatting

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #178](https://github.com/canvist/canvist/pull/178)

Five document tools, each behind its own Cargo feature and included in `full`:

- **`FindAndReplace` (`findAndReplace`).** `setSearchTerm`, `setCaseSensitive`, `nextSearchResult`, `previousSearchResult`, `replace`, `replaceAll`, and the `searchResults` query.
  - The term lives in the editor's settings, outside the document and its history.
  - Matching ignores case unless asked not to.
  - `replaceAll` is one undo step.
- **`TableOfContents` (`tableOfContents`).** The `tableOfContents` query lists headings with their level, text, outline number, and the one the selection is under. `goToHeading` moves the caret to one.
- **`Comments` (`comments`).** A `comment` mark whose `threads` attribute lists thread ids, so comments can overlap.
  - Commands: `setComment` and `unsetComment`. Queries: `activeComments` and `comments`.
  - HTML writes `<span data-type="comment" data-threads="…">`.
  - The StarterKit theme tints commented text.
- **`DragHandle` (`dragHandle`).** `moveBlockUp` and `moveBlockDown` (Alt-↑ and Alt-↓), and `moveBlocksTo` for drops. They move the blocks a text or block selection covers, such as a selected horizontal rule. A block alone in its container moves its container.
- **`ClearFormatting` (`clearFormatting`).** `unsetAllMarks`, `clearNodes` (paragraphs, lifted out of quotes and lists), and `clearFormatting` on `Mod-\`.

#### Add text alignment, typography, character count, and placeholders

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #159](https://github.com/canvist/canvist/pull/159)

Four editing extensions beyond the StarterKit, each behind its own Cargo feature in `canvist_extensions` and part of `full`:

- `TextAlign`: a `textAlign` attribute on paragraphs and headings, with `setTextAlign`, `toggleTextAlign`, `unsetTextAlign`, a `textAlign` query, and `Mod-Shift-l`, `e`, `r`, and `j`. HTML reads and writes TipTap's `style="text-align: …"`.
- `Typography`: TipTap's 22 smart punctuation rules plus Remirror's `<->`, configurable with `without` and `with_output`. Rules that end in a digit fire on the space or punctuation after them.
- `CharacterCount`: `characters` (grapheme clusters), `words`, and `characterLimit` queries, and `setCharacterLimit`. Edits that would go past the limit don't apply.
- `Placeholder`: hint text for empty blocks, with TipTap's `showOnlyCurrent` and `includeChildren`, a `placeholder` query, and `setPlaceholder`.

`canvist_extensions::extras()` lists every extension beyond the StarterKit with its default configuration.

Supporting changes:

- `canvist_core`:
  - `Extension::update_filters` lets an extension refuse a local update after its fix-ups, like TipTap's `filterTransaction`. The update is undone, the command reports that it did not apply, and `Editor::can` answers the same way. The filters are collected when the editor is built, so an editor whose extensions have none pays nothing per update.
  - `EditorState::settings`, `EditorTransaction::set_setting`, and `commands::set_setting` keep per-editor values such as a limit or placeholder text outside the document, its history, and collaboration. Dry runs restore them.
  - Extension metadata now lists only the nodes the built schema has.
  - `InputRule::on` names the typed characters that can complete a rule whose pattern ends in a group or class, so it doesn't run on every punctuation keystroke.
  - Input rules no longer panic when their 64-byte look-back starts inside a multi-byte character, which `**bold**` after a line of `€` or CJK text could trigger.
- `canvist_layout`:
  - Lines align to a block's `textAlign` attribute: left, centre, right, or justified by widening spaces. Server-rendered HTML writes the matching `text-align`.
  - A row now rereads its `dir` and `textAlign` attributes when the block changes. Before, changing `dir` kept the old direction until the row was rebuilt.
  - `DisplayOptions::placeholder` draws placeholder text, faded, in an empty block.
- Clipboard: pasted block attributes keep only the ones the block type declares.

The `@canvist/web` bundle budget moves to 354,000 gzipped bytes, for update vetoes, editor settings, `can()` dry runs, and text alignment, which every host runs.

#### Add subscript, superscript, highlight, and TipTap's text style kit

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #155](https://github.com/canvist/canvist/pull/155)

canvist's first extensions beyond TipTap's StarterKit, each behind its own Cargo feature in `canvist_extensions` (`full` enables every one), with TipTap's JSON names, commands, and keys:

- `Subscript` (`Mod-,`) and `Superscript` (`Mod-.`), which exclude each other.
- `Highlight`: `==text==`, `Mod-Shift-h`, and a `color` attribute (multicolour by default; `Highlight { multicolor: false }` matches TipTap's default).
- `TextStyle` with `Color`, `BackgroundColor`, `FontFamily`, `FontSize`, and `LineHeight`, which each add one attribute to the `textStyle` mark, with set and unset commands and a query for the shared value.

Supporting changes:

- `canvist_model`: `SchemaBuilder::block_attrs` and `mark_attrs` add attributes to types other extensions register, like TipTap's global attributes. `MarkSpec::excludes_registered` excludes marks only when the schema has them. `EmbedSpec::attrs` sets an embed's defaults.
- `canvist_core`: `commands::update_mark_attrs` rewrites one mark's attributes run by run, keeping each run's others.
- `canvist_layout`: `TextStyle::baseline_shift` and `TextPatch::baseline_shift` raise or lower text like CSS `vertical-align`, in canvas layout and server-rendered HTML alike. `AttrBinding::LineHeight` reads a mark's line height. The StarterKit theme styles `subscript`, `superscript`, and `textStyle.backgroundColor` and `lineHeight`. `TextStyle` has a new public field, so code that builds one literally must set `baseline_shift`.
- Clipboard formats: HTML reads and writes `<sub>`, `<sup>`, `<mark data-color>`, and `<span style>`; Markdown writes `==text==` and GitHub's `<sub>` and `<sup>` tags and reads them back. Pasted marks keep only the attributes their type declares.
- `canvist_headless`: the `full` feature registers every extension by name in `Engine::named`, which `canvist_headless_wasm` and the Dart bridge enable. The browser editor still compiles the StarterKit only. `Engine::named` now registers names in one fixed order whatever order they are given in, so the same names always build the same schema and every binding nests and serializes marks alike.
- `@canvist/headless/full` and the Dart `canvist` package export a typed descriptor and handle for each new extension, generated from the new full metadata snapshot. `allExtensions` (TypeScript) and `Extensions.starterKit` (Dart) list them; `starterKit` is unchanged.
- Conformance fixtures can list `extensions` to add to the StarterKit, and every runner supports them.

The StarterKit theme's rules for blocks, marks, and embeds beyond the StarterKit sit behind a new `canvist_layout` feature, `theme-extensions`. `canvist_canvas`, `canvist_canvas_wasm`, and `canvist_paint` turn it on. `@canvist/web` builds without it, and its bundle budget moves to 352,000 gzipped bytes for the engine growth every host shares.

#### Render documents on JavaScript servers with `@canvist/headless/ssr`

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #154](https://github.com/canvist/canvist/pull/154)

`@canvist/headless/ssr` renders a document as semantic HTML and a stylesheet laid out exactly as `@canvist/web` paints it, so a page shows the text before any JavaScript runs and the live editor hydrates from it without moving a pixel. It works in Node, Deno, Bun, and Workers, and so in Next.js server components and Nuxt server routes.

```ts
import { createRenderer } from "@canvist/headless/ssr";

const renderer = await createRenderer({
	theme: { bodyFamily: "Inter, sans-serif" },
});
const { html, css } = renderer.html({ format: "markdown", input: "# Hello" }, {
	width: 640,
});
```

- `renderer.html(content, { width })` returns `{ html, css }`, `renderer.page(content, { width })` a complete page, and `renderer.svg(content, { width })` an SVG image for previews. Content is text in a format or ProseMirror JSON, the same `Content` type `@canvist/web` takes.
- `createRenderer({ extensions, theme, fonts })` takes the browser editor's extensions and `ThemeOptions`, and font files for SVG text. `registerFont(bytes)` adds one later.
- Rendering is a third WebAssembly module, `wasm-ssr/`, so the document entry point stays small.
- `ThemeOptions` and `Content` are now exported by `@canvist/headless`, and `@canvist/web` re-exports them.
- `canvist_layout` gains a `json` feature with `StarterKitOptions::from_json`, which reads the theme options as both packages pass them. `canvist_headless`'s `ssr` feature enables it and re-exports `canvist_layout`.
- The `ssr_page` Rust example is gone: `examples/web` now renders its server-rendered pages with `@canvist/headless/ssr` on each request, including a React page rendered with `renderToString` and hydrated with `hydrateRoot`.

#### Add `canvist_layout`, the layout engine

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #192](https://github.com/canvist/canvist/pull/192)

- Data-driven `Theme` rules for blocks, marks, and embeds, with a default matching TipTap's StarterKit.
- A batched `TextMeasurer` trait with a per-(font, segment) cache, a deterministic `FixedMeasurer`, and a `ParleyMeasurer` behind the `parley` feature.
- UAX #14 line breaking with CSS `pre-wrap` semantics, UAX #9 bidi, tabs, embeds, and hard breaks.
- `DocumentLayout` with container blocks, list markers, viewport virtualization with estimated heights, and scroll anchoring.
- Incremental relayout from a `Commit`. A keystroke re-breaks only the lines around the edit, even in a 100 KB paragraph, and a structural edit re-flattens only the root subtrees it touched.
- Hit testing, caret and selection rectangles, selection bounds for anchoring host UI, and a `LayoutNavigator` for vertical caret motion that implements `canvist_core::LineNavigator` behind the `editor` feature.
- A `DisplayList` with dirty-rectangle damage, plus `to_svg` and `to_html` renderers whose CSS reproduces the canvas line breaks.
- Release-mode performance budget tests and `divan` benchmarks at 100, 10k, and 100k blocks, plus long and heavily formatted paragraphs.

#### Measure with system fonts, and resync a layout without its commits

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/canvist/canvist/pull/161)

- `ParleyMeasurer::with_system_fonts` reads the fonts installed on the machine, for desktop hosts. Registered fonts add families without standing in for the generic ones.
- `DocumentLayout::resync` brings a layout up to date with a document it missed commits for, keeping measurements and the layouts of unchanged blocks.

#### Add images, YouTube and Twitch videos, audio, and file attachments

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #169](https://github.com/canvist/canvist/pull/169)

Five media blocks, each behind its own Cargo feature (`media` enables them all, and `full` includes them). Each is an atomic block whose attributes say what to show. Layout reserves a slot for it, sized by its `height` attribute, and the host draws the picture, player, or file card there:

- `Image` (`image`): `setImage` with `src`, `alt`, `title`, `width`, and `height`. Typing `![alt](src "title")` inserts one, even with typography's curly quotes.
- `Youtube` (`youtube`) and `Twitch` (`twitch`): `setYoutubeVideo` and `setTwitchVideo`, with TipTap's attributes. As in TipTap, they refuse URLs that are not `http(s)` on YouTube's or Twitch's hosts, so a `javascript:` URL never becomes a frame.
- `Audio` (`audio`): `setAudio` with `controls`, `autoplay`, `loop`, and `muted`.
- `File` (`file`): Remirror's attachment card, with `insertFile`, `url`, `fileName`, `fileType`, and `fileSize`.

All five are data-driven by one implementation. Attributes equal to their default are left out, as everywhere else.

- HTML reads and writes TipTap's markup: `<img>`, `<audio>`, and `<iframe>` inside `<div data-youtube-video>` or `<div data-twitch-video>`, with a `title` so assistive technology can name the frame. File cards use `<div data-type="file">`. An image inside a paragraph splits it, and iframes from other sites are dropped.
- Markdown writes and reads images as `![alt](src "title")`; other media become links, a file without a name linked by its URL.
- The StarterKit theme reserves slots for video, audio, and file blocks.

Media commands type their source as a required string in TypeScript and Dart, as Rust already required; before, `setImage({ src: null })` type-checked and failed at runtime. Server-rendered image slots carry `role="img"` and the image's `alt` text as their accessible name. An empty `alt` marks a decorative image, which gets none.

#### Add mentions, hashtags, and emoji on a shared suggestion system

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #166](https://github.com/canvist/canvist/pull/166)

Three extensions, each behind its own Cargo feature and part of `full`, share one suggestion engine, like TipTap's Suggestion utility and Remirror's suggesters. Typing `@`, `#`, or `:` after a space starts a suggestion. A `…Suggestion` query reports `{ char, query }`, a command replaces the trigger with the pick and a space, and Escape or a dismiss command hides it until the trigger is typed again.

- `Mention`: TipTap's `mention` embed with `id`, `label`, and `mentionSuggestionChar`, plus `insertMention`, `dismissMentionSuggestion`, and `mentionSuggestion`. Backspace after a mention turns it back into its trigger.
- `Hashtag`: a `hashtag` mark with a `tag`, marked as you type and paste, with `insertHashtag`, `dismissHashtagSuggestion`, `hashtagSuggestion`, and a `hashtags` list query.
- `Emoji`: TipTap's `emoji` embed with `name`, plus canvist's `emoji` attribute holding the character. It covers GitHub's 1,870 gemoji, `:shortcode:` as you type, `setEmoji`, `dismissEmojiSuggestion`, `emojiSuggestion`, and `searchEmoji({ query, limit })`.

Supporting changes:

- `canvist_model`: `Value::Map` holds nested attributes. JSON objects in attributes become maps instead of `null`, and they round-trip.
- `canvist_core`: `ValueType::List` and `ValueType::Record` describe structured query answers. Bindings generate typed lists and records from them.
- `canvist_collab`: maps in attributes travel through Yjs instead of being dropped.
- `canvist_layout`: `EmbedStyle::Label` gains a `fallback` attribute, drawn when the label's attribute is missing: after the prefix, or as `:value:` without one. The StarterKit theme draws an emoji without its character as `:name:` and a mention without a label as `@id`, as every export does. Code that builds a `Label` literally must set `fallback`.
- `canvist_headless`: query and command arguments accept JSON objects.
- Clipboard: HTML reads and writes TipTap's mention and emoji spans and canvist's hashtag spans. A mention without a label reads back without one. Plain text and Markdown write what a mention or emoji shows.
- `@canvist/headless` types list and record answers. The Dart `canvist` package answers with typed records, such as `({String char, String query})`, and its queries take arguments.

The `@canvist/web` bundle budget moves to 356,000 gzipped bytes and its wasm budget to 342,000, for JSON object and list values and list and record argument types, which every host runs.

The StarterKit theme's mention rule moves behind `canvist_layout`'s `theme-extensions` feature, beside the emoji rule, so hosts that compile only the StarterKit, such as `@canvist/web`, carry neither.

The Dart package's Flutter editor views turn on `canvist_layout`'s `theme-extensions` feature, so they style the extensions the package ships.

#### Add tables

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #177](https://github.com/canvist/canvist/pull/177)

`Table` (feature `table`, included in `full`) is TipTap's TableKit. It adds a `table` of `tableRow`s holding `tableCell`s and `tableHeader`s, each cell holding blocks, with TipTap's `colspan`, `rowspan`, and `colwidth` attributes.

- **Commands:**
  - `insertTable`;
  - `addColumnBefore`, `addColumnAfter`, `deleteColumn`;
  - `addRowBefore`, `addRowAfter`, `deleteRow`;
  - `deleteTable`, `mergeCells`, `splitCell`, `mergeOrSplit`;
  - `toggleHeaderRow`, `toggleHeaderColumn`, `toggleHeaderCell`;
  - `setCellAttribute`, `goToNextCell`, `goToPreviousCell`, `fixTables`.
- **Query:** `tableCell` returns where the selection head sits, with its spans and the table's size.
- **Selections:** a selection from one cell to another selects the rectangle of cells between them.
- **Keys:** Tab moves to the next cell and adds a row past the last. Backspace and Delete never join text across a cell's edge, and deleting a selection across cells clears them.
- **Headers:** toggling the header row or column leaves the corner cell to the other one when it is on, as TipTap does, so toggling twice changes nothing.
- **Repairs:** short rows are padded with empty cells after every structural change, including a gap left of a cell that spans down from the row above.

`canvist_model` adds `TableMap`, which places a table's cells on its grid from their spans.

Layout places tables on a grid:

- Columns take their `colwidth`, or an even share of what is left.
- Each row is as tall as its tallest cell, and spanning cells grow their last row.
- Grid lines are drawn as CSS's collapsed borders.
- Hit testing, arrow keys, incremental relayout, and splicing all follow the grid.
- A keystroke in a table of 1,000 cells relays out in about 7 µs.

The HTML renderer writes a fixed-layout `<table>` with the same column widths, and gives header cells a `scope`: `col` across the first row and `row` down the first column. The StarterKit theme draws hairlines and tints header cells.

HTML reads and writes `<table>` as TipTap does, including bare text in cells and a row that a merged cell spans entirely. Markdown writes a GitHub pipe table when the table fits one exactly, and HTML when it does not. It reads both back, with column alignment.

Grid layout sits behind a new default `canvist_layout` feature, `tables`, which `canvist_canvas`, `canvist_canvas_wasm`, and `canvist_paint` turn on. `canvist_web` builds without it, and the `@canvist/web` bundle budget moves to 357,000 gzipped bytes and its wasm budget to 343,000, for the grid hooks every host carries.

#### Fit the web editor in its 350 KB budget

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` now ships 349.8 KB gzipped, measured as an app bundles it (`npm run size`, enforced in CI). The WebAssembly module is optimized without inlining, which gzips smaller.
- `canvist_layout::css_number` formats CSS numbers without `core`'s float formatting, which no longer links into the module.
- `EditorState::change` and `InputRule::new` keep one copy of their logic instead of one per command and rule, and `Engine::metadata` is built on first use.

#### Add `@canvist/web`, the canvas editor for the browser, with a fonts plugin

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` and `canvist_web` paint the document on an opaque Canvas2D context with HiDPI backing stores, dirty-rectangle repaints, and baselines and backgrounds snapped to the browser's own pixel grid. `createEditor` types commands, queries, and plugin APIs from what you register.
- Text is measured by the browser: `measureText` advances with kerning placed as shapers place it, line boxes read from the DOM, family lists resolved to the face the DOM uses, and DOM ranges for Arabic and for Firefox's fractional sizes. The canvas wraps where server-rendered HTML does.
- `@canvist/web/fonts` loads faces from URLs or bytes with `unicode-range` and `font-display`, builds metric-compatible fallbacks, lists local fonts through `queryLocalFonts`, and writes the same faces as `@font-face` CSS for SSR.
- `canvist_layout`: `TextMeasurer` batches carry line-box requests that a measurer may answer; `DocumentLayout::forget_fonts` relays out only the blocks a loaded face affects; `measure_stats`, `line_ranges`, `fonts`, and `font_metrics`; kerning between adjacent segments; `chromium_breaks`; border widths snapped to device pixels; `Theme::starter_kit_with` for dark and custom-font themes, and `Chrome::background`. `to_html` now sets the background and `text-spacing-trim: normal`, the StarterKit code stack no longer names `ui-monospace`, and graphemes come from the editor's segmenter. `ParleyMeasurer` sets a variable font's optical size to the font size, as browsers do.
- `canvist_headless`: `Doc::edit` runs editor calls with a host's clock and line navigator, and `Doc::command_named` resolves a named command.
- `@canvist/headless` exports its generated extension descriptors at `@canvist/headless/extensions`, with no WebAssembly.

### 🐛 Fixed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)

#### Keep `@canvist/web` lean when it compiles only the StarterKit

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #179](https://github.com/canvist/canvist/pull/179)

`@canvist/web` compiles the StarterKit alone. It now carries no code for extensions its schema cannot reach:

- Clipboard codecs and StarterKit theme rules for extension blocks and marks compile only with their features.
- Table grids, ink drawings, and kinetic motion compile only with `canvist_layout`'s `tables`, `ink`, and `kinetic` features. Layout's hot paths read a row's grid part through accessors that are always `None` without `tables`.
- The editor builds its extension metadata the first time it is read, not on every build. The browser editor never reads it.

Across the extension layers, its bundle budget in `tools/bench/size.json` moves from 350,000 to 359,000 gzipped bytes, and its wasm budget from 340,000 to 345,000, for engine growth every host shares:

- JSON object and list values in attributes and command arguments.
- List and record argument types.
- The clipboard core's new block and mark handling.
- Extensions vetoing updates, `can()` dry runs, and editor settings.
- Text alignment and direction in layout.

<details>
<summary><strong>🧪 Testing</strong></summary>

#### Test canvas and server-rendered HTML for pixel parity in every browser

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #138](https://github.com/canvist/canvist/pull/138)

- Playwright renders a fixture matrix (system and web fonts, a variable font, CJK with `unicode-range` subsets, Arabic and Hebrew, Devanagari, three emoji formats, letter spacing, sizes from 10 to 64 px) as the canvas, the SSR HTML, and SVG, at device pixel ratios 1 to 3, fractional zoom, and light and dark themes, in Chromium, Firefox, and WebKit on macOS and Linux.
- Line boxes, runs, and caret stops are compared exactly; pixels with pixelmatch and SSIM against per-browser limits, with every known difference documented. Each run uploads a side-by-side report.
- A Rust test holds `parley`, with the same Inter file, to Chromium's line breaks.

</details>
