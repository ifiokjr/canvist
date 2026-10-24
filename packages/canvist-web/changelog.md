All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Collaborate over y-websocket out of the box with `@canvist/web/collab`

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #165](https://github.com/canvist/canvist/pull/165)

`@canvist/web/collab` joins a browser editor to a room on any server that speaks y-websocket's protocol: y-websocket's own server, canvist's `receiveSync` servers, or canvist Cloud Sync. The editor is the Yjs peer itself, so there is no `Y.Doc` to keep in step, and JavaScript Yjs peers in the room see the same document.

```ts
import { connect, createEditor } from "@canvist/web/collab";

const editor = await createEditor(element);
const connection = connect(editor, {
	url: "wss://sync.example.com",
	room: "notes",
});
await connection.whenSynced();
```

- `createEditor` from `@canvist/web/collab` loads a second WebAssembly module (`wasm-collab/`) with Yjs inside, so editors that don't collaborate never download it. `loadCollab(source)` loads it from elsewhere.
- `connect(editor, { url, room, params, clientId, reconnect, WebSocket })` returns a `Connection` with `status`, `synced`, `clientId`, `subscribe`, `whenSynced`, and `disconnect`. It reconnects with backoff and catches up both ways.
- Undo is per person: `Mod-z`, `editor.undo()`, and the `undo` command revert only this peer's changes, grouped by word as when editing alone.
- `canvist_headless`: `join_message()` and `Engine::join_sync` join a shared document over y-sync with no state of its own; `Doc::receive_sync_changes` also returns the commit of each remote change; `Doc::history_step` undoes or redoes and returns the update; `Doc::shared_history_key` and `Doc::shared_history_command` identify History's keys and commands on a shared document, which `press` and `run_named` now answer with the shared history. `Doc::edit` joins any follow-up the shared state needed to the update it returns, and integrating a collaborator's change no longer advances the document's clock (y-websocket echoes each update to its sender, which split typing into one undo step per key).
- `canvist_core`: `UndoGrouping` is the rule `UndoHistory` groups changes by, on its own, so a shared history can group the same way.

#### Edit on the web: EditContext input, IME, clipboard, drag and drop, accessibility

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #140](https://github.com/canvist/canvist/pull/140)

- Text arrives through `EditContext` in Chromium, with the paragraph and caret bounds the platform's input methods need, and through a hidden textarea everywhere else. `input: "textarea"` forces the textarea, as a kill switch for `EditContext` problems.
- Compositions from input methods and dead keys show as preedit text and commit in place. Key bindings follow the platform (`Mod` is Cmd on Apple devices), detected from the browser or set with `platform`.
- Click, Shift-click, double-click, and triple-click select; dragging extends by the unit the press selected. Copy, cut, and paste carry HTML and plain text; the selection can be dragged to move or copy it, and drops from other applications land where they fall.
- A visually hidden copy of the document, the same semantic HTML the server renders, is refreshed on idle for screen readers and find-in-page.
- Every input event is painted in its own task. `editor.inputLatency()` reports input-to-paint times; the browser tests hold their 95th percentile under 8 ms and type a 1,000-key burst without dropping a key.
- `Engine::with_platform` in `canvist_headless` chooses what `Mod` means in key bindings.

#### Read-only editors, placeholders, and menus that follow the selection

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #156](https://github.com/canvist/canvist/pull/156)

`@canvist/web` gains the editor features TipTap users reach for first.

- **`editable`**: `createEditor(element, { editable: false })` and `editor.setEditable(editable)` make an editor read-only. People can still select with the pointer and navigation keys and copy, but typing, pasting, cutting, and dropping change nothing, no caret shows, and the field has `aria-readonly`. Commands and the API still change the document, as in TipTap. `editor.editable` reads it.
- **`placeholder`**: dimmed text where the first line starts while the document is empty, set in the body font and line height so it sits where typing begins, and the field's `aria-placeholder`. `editor.setPlaceholder(text)` changes it, and `editor.isEmpty` reports an empty document.
- **`@canvist/web/menus`**: `bubbleMenu(editor, element, options)` keeps an element over selected text, and `floatingMenu` beside the caret on an empty line. They follow edits, scrolling, and resizing, flip to the other side when there is no room, keep the editor's focus when pressed, and take `placement`, `offset`, `padding`, and `shouldShow`. `placeMenu(element, rect, options)` is the positioning on its own.
- **Selection geometry**: `editor.selectionRect()` is the selection's bounding box in document coordinates, `editor.toClientRect(rect)` converts to viewport coordinates, and `editor.blockText()` is the text of the block holding the selection. `caretRect()` and `selectionRect()` now lay out first when an edit is waiting for the next frame, so they describe the edited document.
- **Focus**: `editor.focused` reads focus, and `subscribe` listeners also hear about focus and `editable` changes.
- `@canvist/react` and `@canvist/vue`: `CanvistEditor`'s `editable` and `placeholder` props follow their values, and `BubbleMenu` and `FloatingMenu` components wrap the menus.

#### React and Vue bindings, and hydrating server-rendered HTML

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #141](https://github.com/canvist/canvist/pull/141)

- `@canvist/react` adds `useCanvistEditor`, `useEditorState` (through `useSyncExternalStore`), and `<CanvistEditor />`. `@canvist/vue` adds the same composables and a component that emits `ready` and `update`. Both are typed from the extensions and plugins you register.
- `editor.subscribe(listener)` in `@canvist/web` calls back after every document or selection change.
- `createEditor` hydrates server-rendered HTML: given an element holding canvist's `.cv-doc` markup, it reads the document from it, keeps it on screen until fonts settle, and swaps in the canvas in one frame, with no layout shift.
- `examples/web` shows a plain page, React, Vue, and a page rendered by the new `ssr_page` example in `canvist_headless`, with Playwright tests in every browser.

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

#### Screen readers meet the text field first, and placeholders have 4.5:1 contrast

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #168](https://github.com/canvist/canvist/pull/168)

- In Firefox and Safari, where text arrives through a hidden textarea, the textarea now sits right after the canvas, as EditContext's field does in Chromium, so screen readers reach the editor's text field before its document mirror.
- The placeholder shows the body text's colour at 62% instead of 50%, which gives 4.5:1 contrast on the StarterKit's light and dark pages.
- New browser tests: an axe audit (WCAG 2.2 AA) of editable, read-only, empty, and menu-showing editors; virtual screen reader smoke tests; and Safari composition, Android keyboard, and touch input sequences in Chromium, Firefox, and WebKit.

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

#### Fit the web editor in its 350 KB budget

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` now ships 349.8 KB gzipped, measured as an app bundles it (`npm run size`, enforced in CI). The WebAssembly module is optimized without inlining, which gzips smaller.
- `canvist_layout::css_number` formats CSS numbers without `core`'s float formatting, which no longer links into the module.
- `EditorState::change` and `InputRule::new` keep one copy of their logic instead of one per command and rule, and `Engine::metadata` is built on first use.

<details>
<summary><strong>📖 Documentation</strong></summary>

- **Mark the browser platforms Available.** The web, React, Vue, server-side rendering, and design canvas quick starts, the platform tables in the readme and the docs, the agent guide and skill, and the website's platform list now say "Available". The package readmes describe read-only editors, placeholders, menus, collaboration, server rendering, and the bundle budget. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #173](https://github.com/canvist/canvist/pull/173)

</details>

<details>
<summary><strong>⚡ Performance</strong></summary>

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

- **Drop metadata descriptions from the browser editor's module.** `@canvist/web` builds its WebAssembly module with `--cfg canvist_omit_docs`, which leaves the command, query, and argument descriptions out of `canvist_core`'s metadata. Only bindings that generate types read them, and the browser module never does. With `wasm-opt --converge`, the editor ships 349.1 KB gzipped, down from 349.97 KB, under its 350 KB budget. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #154](https://github.com/canvist/canvist/pull/154)

</details>

<details>
<summary><strong>🔨 Refactor</strong></summary>

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

</details>
