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

#### Ship every extension in `@canvist/headless/full`, a second WebAssembly module

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #169](https://github.com/canvist/canvist/pull/169)

`@canvist/headless` compiles in TipTap's StarterKit only, so its module stays at 1,060 KB raw (444 KB gzipped), inside its 1,200 KB budget. Every other extension is in `@canvist/headless/full`, a second module with the same API, as `@canvist/headless/scene` is for canvas scenes:

```ts
import { createEngine, highlight, starterKit } from "@canvist/headless/full";

const engine = await createEngine([...starterKit, highlight]);
```

- `@canvist/headless` exports the core and StarterKit descriptors. Its `createEngine` only accepts StarterKit extensions, so passing another one is a type error. `@canvist/headless/full` also exports every other descriptor, `extras`, and `allExtensions`.
- The emoji table is data rather than code. The full module loads it the first time an engine registers `emoji`, so neither module compiles it in.
- `bench/overhead.ts` checks both modules: 1,200 KB for the StarterKit one and 1,400 KB for the full one, which comes to 1,321 KB (551 KB gzipped).
- `canvist_extensions` adds the `emoji-data` feature, which compiles the gemoji table in, and `extras`, which is `full` without it. Without `emoji-data`, `emoji::load` supplies the table at runtime, and `emoji::is_loaded` says whether one is. `canvist_headless` passes `extras` through. `full` still includes everything, so the Dart package keeps the table compiled in.

#### Load, transform, and export canvas scenes on a server

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #136](https://github.com/canvist/canvist/pull/136)

`canvist_headless`'s new `scene` feature adds `SceneEngine` and `SceneDoc`. They load canvas scenes from JSON (flat or nested) or Yjs state, run canvas commands, edit inspector fields, transform scenes in transactions with undo, and export the page or chosen nodes as SVG or PNG with the renderer the canvas draws with. With `collab`, a scene shares and merges Yjs updates. Scene failures carry the existing stable error codes.

`@canvist/headless/scene` exposes the same in TypeScript, with `SceneJson` types for every node, property, paint, effect, and shape. It is a separate WebAssembly module, so the document entry point's size is unchanged.

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

#### Add `@canvist/headless`, the document model for JavaScript runtimes with no DOM

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #110](https://github.com/canvist/canvist/pull/110)

Add `@canvist/headless` (`packages/canvist-headless`), canvist's document model for JavaScript runtimes with no DOM, over a new `canvist_headless_wasm` crate (wasm-bindgen, no `web-sys`).

- Loads in Node, Deno, and Bun from disk, and in Workers from an imported `WebAssembly.Module`. A `build` step emits JavaScript for npm, and CI installs the packed tarball as a dependency.
- Types come from the extension metadata (`src/extensions.generated.ts`, generated from the `canvist_extensions` snapshot): an engine's documents only offer the registered extensions' commands, queries, blocks, and marks, with argument types such as heading levels `1 | … | 6`. `tests/types.check.ts` pins this with `tsc --noEmit` and `@ts-expect-error`.
- The shared conformance fixtures pass in Node and Deno (30 of 30), alongside API tests that sync with JavaScript Yjs.
- `examples/server` is a Deno server that converts Markdown to JSON over HTTP and runs a y-websocket-compatible sync endpoint, tested end to end with real Yjs clients.
- `bench/overhead.ts` records per-call overhead and the wasm size against budgets.

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

#### A native rich-text editor for React Native

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #193](https://github.com/canvist/canvist/pull/193)

`@canvist/react-native` puts the iOS and Android `CanvistView`s in React Native apps as a Fabric component, not a WebView, so typing, input methods, autocorrect, dictation, selection handles, VoiceOver, and TalkBack work as they do in the native editors. Commands, queries, and documents are typed from the extensions you register, with the same API as `@canvist/web`, and calls go through a synchronous TurboModule, so a query answers at once:

```tsx
const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
const isBold = useEditorState(editor, (e) => e.query.isBold(), false);

<CanvistView {...props} style={{ flex: 1 }} />;
editor?.commands.toggleHeading({ level: 2 });
editor?.commands.toggleItalic(); // type error: italic is not registered
```

`CanvistEditor` wraps both in one component with `onReady` and `onUpdate`. A call from JavaScript into the editor takes 6 to 47 µs at p95 on an iOS Simulator and 210 to 344 µs on an Android emulator (Release builds); keystrokes never cross into JavaScript. The example app in `examples/react-native` is a notes screen with a formatting toolbar, and its Maestro flows run on a simulator and an emulator in CI.

- `@canvist/headless/native` is the typed editor over any host that runs canvist natively: a small synchronous bridge of names and JSON, with failures rethrown as `CanvistError`s. `@canvist/lynx` will share it.
- The Swift and Android libraries gain what JavaScript hosts share: `CanvistHostView`, a `CanvistView` started from an element's first props and reporting changes and focus; `HostedEditors`, a thread-safe registry of editors by id with failures as `[canvist:<code>] <reason>`; JSON ranges (`selectionJSON()`, `select(rangeJSON:)`); `CanvistView.onFocusChange`; and `ExtensionSet(names:)`.
- `platforms/apple/Canvist.podspec` builds the Swift package with CocoaPods.
- `canvist-android` compiles for the Kotlin 2.2 language and standard library, so apps on Kotlin 2.2, React Native 0.88's version, can use it.

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

#### Add `@canvist/web`, the canvas editor for the browser, with a fonts plugin

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` and `canvist_web` paint the document on an opaque Canvas2D context with HiDPI backing stores, dirty-rectangle repaints, and baselines and backgrounds snapped to the browser's own pixel grid. `createEditor` types commands, queries, and plugin APIs from what you register.
- Text is measured by the browser: `measureText` advances with kerning placed as shapers place it, line boxes read from the DOM, family lists resolved to the face the DOM uses, and DOM ranges for Arabic and for Firefox's fractional sizes. The canvas wraps where server-rendered HTML does.
- `@canvist/web/fonts` loads faces from URLs or bytes with `unicode-range` and `font-display`, builds metric-compatible fallbacks, lists local fonts through `queryLocalFonts`, and writes the same faces as `@font-face` CSS for SSR.
- `canvist_layout`: `TextMeasurer` batches carry line-box requests that a measurer may answer; `DocumentLayout::forget_fonts` relays out only the blocks a loaded face affects; `measure_stats`, `line_ranges`, `fonts`, and `font_metrics`; kerning between adjacent segments; `chromium_breaks`; border widths snapped to device pixels; `Theme::starter_kit_with` for dark and custom-font themes, and `Chrome::background`. `to_html` now sets the background and `text-spacing-trim: normal`, the StarterKit code stack no longer names `ui-monospace`, and graphemes come from the editor's segmenter. `ParleyMeasurer` sets a variable font's optical size to the font size, as browsers do.
- `canvist_headless`: `Doc::edit` runs editor calls with a host's clock and line navigator, and `Doc::command_named` resolves a named command.
- `@canvist/headless` exports its generated extension descriptors at `@canvist/headless/extensions`, with no WebAssembly.

### 📝 Changed

- **Keep descriptions out of the generated extension descriptors.** The descriptors in `@canvist/headless/extensions` no longer carry each command, query, and field's `doc` as data: it is a comment above each entry in the generated file. Types never read it, and as data it shipped in every browser bundle (627 bytes gzipped of `@canvist/web`). `doc` is now optional on `CommandSpec`, `QuerySpec`, and `FieldSpec`, and `engine.metadata()` is typed as `ExtensionDescriptor[]`, with every description, for tools such as agent tool schemas that need them. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #156](https://github.com/canvist/canvist/pull/156)

### 🐛 Fixed

#### Package `@canvist/canvas` for npm, starting from your own scene

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #170](https://github.com/canvist/canvist/pull/170)

`@canvist/canvas` is now an npm package like the other browser packages: a `package.json` with `repository`, `files`, and `exports`, a TypeScript build into `dist/`, and a tarball the release workflow checks. It is versioned with the SDK, and publishes once its npm name is bootstrapped and its trusted publisher set up.

- `mountCanvas(canvas, { scene })` starts from scene JSON, nested or flat, instead of always the demo scene. It defaults to one empty page; `demo: true` keeps the demo. Peers that start from the same scene share the same state, so merging them duplicates nothing.
- The handle gains `toJson()`, `encodeState()`, and `applyUpdate(update)`, to store a scene, start another canvas from it, and merge stored state.
- `canvist_canvas_wasm`: `App::with_scene(client, &scene)` and `App::encode_state()`, and `CanvasApp.create` takes the starting scene. `SceneJson` is re-exported from `@canvist/headless/scene`.
- `@canvist/headless`: `Registered`, `StarterKit`, and `Content` are now declared in its wasm-free type module, so a package that only needs the scene types (such as `@canvist/canvas`) type-checks without the WebAssembly built. Every entry point still exports them.

- **Read the bundled WebAssembly module from a file path.** `load()` converts the bundled module's `file:` URL to a path before reading it, so the package type-checks under JSR's `deno publish` as well as with `@types/node`. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #125](https://github.com/canvist/canvist/pull/125)
- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
