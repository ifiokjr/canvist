All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Add `canvist_model`, the block-based document model

- `Schema` built from block, mark, and embed specs that extensions contribute.
- `Document` with stable `BlockId`s, nested containers, and `InlineText` that stores formatting as mark runs and embeds as single characters.
- Invertible `Step`s with point mapping. `Transaction` applies them atomically with rollback, and `Commit` records which blocks changed for incremental layout.
- ProseMirror/TipTap-compatible JSON import and export.
- Property tests for invariants and exact undo, a 20,000-keystroke typing stress test, and `divan` benchmarks covering 100 to 100,000 blocks.

#### Add `canvist_core`, the platform-agnostic editing engine

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #90](https://github.com/canvist/canvist/pull/90)

Add `canvist_core`, the platform-agnostic editing engine, and `Transaction::allocate_id` in `canvist_model` so commands can create blocks inside a transaction.

- `EditorState` holds the document, selection, stored marks, and IME composition. Every change goes through `EditorState::change` or `EditorState::apply_steps`, which map the selection and composition, clear stale stored marks, and keep the document non-empty.
- Commands as plain functions: typing with mark inclusivity, deleting by grapheme, word, line, or document (joining blocks and lifting out of containers at block boundaries), marks, select all, block selections, and IME composition.
- Selection motion by grapheme cluster and word (UAX #29), line, and document, treating embeds and atomic blocks as single stops. Vertical motion goes through the `LineNavigator` trait, with a paragraph-based fallback for headless use.
- An outline notation for before/after command tests.

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

#### Add the headless, typed extension API (architecture Decision 8)

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #102](https://github.com/canvist/canvist/pull/102)

- `canvist_model`: `Document::savepoint`, `rollback_to`, and `release`. A rollback restores the content, the version counter, every block revision, and id allocation exactly, so a dry run leaves layout caches warm. Savepoints nest.
- `canvist_core`:
  - `EditorBuilder::add` returns an extension's typed handle, behind `TypedExtension` and an unforgeable `Registration`.
  - `CommandSpec` and `QuerySpec` declare named commands and queries with typed, checked arguments (`Field`, `ValueType`). `Editor::run_named`, `command_named`, and `query` serve dynamic hosts, and `Editor::metadata` and `metadata_json` describe every node, attribute, command, and query for binding generators.
  - `Editor::can` and `EditorState::dry_run` answer whether a command would apply without changing anything. `Editor::chain` runs commands all or nothing as one undo step, and `EditorState::atomic` backs it.
  - `Extension::commands` now returns `CommandSpec`s, and `queries` and `nodes` are new.
  - Input rules run only when the typed character can complete their pattern, and unanchored rules search a bounded `Lookback` (64 bytes by default, or the current word for autolinks). A keystroke at the end of a long line no longer scans 500 bytes per rule.
- `canvist_extensions`: every extension has a typed handle (`HeadingHandle::toggle(HeadingLevel::H2)`, `MarkHandle::is_active`, `LinkHandle::href`, ...), argument-checked commands, queries, and node metadata. `StarterKit::add_all` returns all the handles.

### 🐛 Fixed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
