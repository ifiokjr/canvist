All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.1.1](https://github.com/canvist/canvist/releases/tag/v0.1.1) (2026-10-20)

### 🚀 Feature

#### Render the canvas, and edit text frames with the real text editor

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #134](https://github.com/canvist/canvist/pull/134)

`canvist_paint` (new) draws a canvas page through one `Painter` contract. Backends:

- vello on wgpu: WebGPU in browsers, and Metal, Vulkan, and DX12 natively;
- `vello_cpu`, for PNG export and the browser fallback;
- SVG, with text kept as text.

The scene walker culls to the viewport and has level of detail. Text is shaped with parley and the bundled Atkinson Hyperlegible font, so a frame wraps the same on the canvas, in a PNG, and in SVG. With 10,000 nodes on screen, encoding a GPU frame takes about 1 ms and a 1080p CPU frame about 4 ms.

`canvist_canvas` adds `TextSession` (feature `editor`). It opens a text frame in a `canvist_editor::Editor`, places the caret where you click, draws the caret and selection in the overlay, and hands the document back on close. It also adds `Canvas::tool_named`.

`canvist_editor` adds `Editor::into_state`, so a host that lent a document to an editor can take it back.

`canvist_canvas_wasm` (new, unpublished) runs the canvas in the browser on WebGPU with a CPU fallback, behind the `@canvist/canvas` demo.

#### Add `canvist_editor`, the platform-agnostic editing engine

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #90](https://github.com/canvist/canvist/pull/90)

Add `canvist_editor`, the platform-agnostic editing engine, and `Transaction::allocate_id` in `canvist_model` so commands can create blocks inside a transaction.

- `EditorState` holds the document, selection, stored marks, and IME composition. Every change goes through `EditorState::change` or `EditorState::apply_steps`, which map the selection and composition, clear stale stored marks, and keep the document non-empty.
- Commands as plain functions: typing with mark inclusivity, deleting by grapheme, word, line, or document (joining blocks and lifting out of containers at block boundaries), marks, select all, block selections, and IME composition.
- Selection motion by grapheme cluster and word (UAX #29), line, and document, treating embeds and atomic blocks as single stops. Vertical motion goes through the `LineNavigator` trait, with a paragraph-based fallback for headless use.
- An outline notation for before/after command tests.

#### Add the `Editor`: history, keymaps, input and paste rules, and the `Extension` trait

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #92](https://github.com/canvist/canvist/pull/92)

- `Editor` and `EditorBuilder` assemble the schema, commands, key bindings, and rules from extensions, sorted by priority, and route every update through one place that runs extension fix-ups and records history.
- `History` is the boundary the collaboration crate replaces; `UndoHistory` groups typing (including IME composition) and deletions by time (500 ms by default), keeps other edits as their own steps, restores the selection, and limits depth.
- `Keymap` parses ProseMirror-style bindings such as `Mod-Shift-z`, with `Mod` meaning Cmd on Apple platforms and Ctrl elsewhere. `BaseKeymap` binds each platform's standard caret, deletion, Enter, select-all, and Escape keys.
- `InputRule` rewrites text as it is typed (block type, wrapping, marks, text replacement, and atomic blocks), runs only on whitespace or punctuation, and skips code blocks. Backspace right after a rule restores the typed text. `PasteRule` formats pasted text, for example into links.

#### Settle the `LineNavigator` interface that `canvist_layout` implements

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #103](https://github.com/canvist/canvist/pull/103)

- `LineNavigator` now has `vertical(&mut self, doc, from: Caret, direction: VerticalDirection, goal_x: Option<f32>) -> Result<Option<VerticalMotion>, NavigationError>` and `line_edge(&mut self, doc, at: Caret, edge: LineEdge) -> Result<Caret, NavigationError>`. Methods take `&mut self` so layout can measure blocks lazily, and return errors instead of guessing when it cannot.
- `Caret` pairs a point with an `Affinity` (`Upstream` or `Downstream`), so the offset where a line soft-wraps can be the end of one line or the start of the next. `EditorState::affinity` tracks it: line and vertical motion set it, and every other change resets it to `Downstream`.
- `Env` holds the navigator mutably. Build one with `Env::new(now, &mut navigator)`, or `Env::headless(now)` for the `ParagraphNavigator` fallback. `Editor` methods now take `&mut Env`.
- `EditorError::Navigation` carries a navigator's error.

#### Add structural editing commands to `canvist_editor`

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #91](https://github.com/canvist/canvist/pull/91)

- Enter splits text blocks (a heading continues as a paragraph at its end), and Backspace and Delete join blocks across containers, adopting the rest of a joined list item or quote.
- Block types, block attributes, wrapping and lifting (such as blockquotes), and atomic blocks such as horizontal rules.
- Lists generic over list and item types: split, indent, outdent, join with the previous item, wrap, and toggle, converting between list types in place while keeping block ids.
- `Slice` for copy, cut, and paste: copying keeps partly selected containers around their children, and pasting merges open text at both ends and converts or flattens blocks the destination does not accept.
- Property tests that random command sequences keep every invariant and that every update's inverse restores the document exactly.

#### Add `canvist_headless`, the document model and editing engine with no UI

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #108](https://github.com/canvist/canvist/pull/108)

Add `canvist_headless`, the canvist document model and editing engine with no UI, for servers, scripts, and tests.

- `Engine` registers extensions once (with typed handles) and is `Send + Sync`; each `Doc` is `Send`. Load and export ProseMirror/TipTap JSON, HTML, Markdown, and plain text, run typed or named commands against explicit selections, answer queries, paste, undo, and take `Snapshot`s.
- `Position` addresses text by block path and UTF-16 offset, the same in every language. `ErrorCode` gives every failure a stable name.
- The `collab` feature makes a `Doc` a Yjs peer and a y-sync server endpoint (`receive_sync` returns the reply and what to relay), and `validate` reports shared content the schema rejects. The `ssr` feature adds a `Renderer` for HTML and SVG through `canvist_layout` and `parley` with a bundled font. The default build has neither.
- `conformance/` holds language-neutral fixtures (input, operation script, expected output) that every binding runs; the Rust runner passes all 30.
- `canvist_editor`: `Editor::apply_external` and `EditorState::apply_external` let a collaboration binding change the editor's own document, so a shared document is held once.
- `canvist_collab`: `Binding::rejected_blocks` counts shared items the schema hides, and the `sync` feature makes `Binding` and `SyncSession` `Send + Sync`.

#### Add `canvist_extensions`, the StarterKit, with one Cargo feature per extension

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #93](https://github.com/canvist/canvist/pull/93)

Add `canvist_extensions`, the StarterKit, with each extension behind its own Cargo feature.

- Nodes: `Document`, `Paragraph`, `Heading`, `Blockquote`, `CodeBlock`, `HardBreak`, `HorizontalRule`, `BulletList`, `OrderedList`, `ListItem`, and `TaskList`, with TipTap's names, key bindings, and Markdown-style input rules.
- Marks: `Bold`, `Italic`, `Underline`, `Strike`, `Code`, and a non-inclusive `Link` with autolinking and linking on paste.
- Functionality: `History` keys and `TrailingNode`.
- `canvist_editor` gains `UndoInputRule` (Backspace takes back an input rule before any other Backspace binding), `lift_empty_block`, and a public `wrap_in_list`, and its hot paths avoid scanning sibling lists so a keystroke costs the same at 100 or 100,000 blocks.
- A scripted-typing harness (`"## Hi⏎- one⏎two⇥"`) with JSON snapshots, property tests that random keystroke scripts keep every invariant and undo and redo exactly, and `divan` keystroke benchmarks at 100, 10,000, and 100,000 blocks.

#### Add the headless, typed extension API (architecture Decision 8)

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #102](https://github.com/canvist/canvist/pull/102)

- `canvist_model`: `Document::savepoint`, `rollback_to`, and `release`. A rollback restores the content, the version counter, every block revision, and id allocation exactly, so a dry run leaves layout caches warm. Savepoints nest.
- `canvist_editor`:
  - `EditorBuilder::add` returns an extension's typed handle, behind `TypedExtension` and an unforgeable `Registration`.
  - `CommandSpec` and `QuerySpec` declare named commands and queries with typed, checked arguments (`Field`, `ValueType`). `Editor::run_named`, `command_named`, and `query` serve dynamic hosts, and `Editor::metadata` and `metadata_json` describe every node, attribute, command, and query for binding generators.
  - `Editor::can` and `EditorState::dry_run` answer whether a command would apply without changing anything. `Editor::chain` runs commands all or nothing as one undo step, and `EditorState::atomic` backs it.
  - `Extension::commands` now returns `CommandSpec`s, and `queries` and `nodes` are new.
  - Input rules run only when the typed character can complete their pattern, and unanchored rules search a bounded `Lookback` (64 bytes by default, or the current word for autolinks). A keystroke at the end of a long line no longer scans 500 bytes per rule.
- `canvist_extensions`: every extension has a typed handle (`HeadingHandle::toggle(HeadingLevel::H2)`, `MarkHandle::is_active`, `LinkHandle::href`, ...), argument-checked commands, queries, and node metadata. `StarterKit::add_all` returns all the handles.

### 🐛 Fixed

#### Add clipboard formats to `canvist_extensions` behind the `clipboard` feature

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #94](https://github.com/canvist/canvist/pull/94)

Add clipboard formats to `canvist_extensions` behind the `clipboard` feature (part of `starter-kit`).

- `clipboard::to_html`, `to_markdown`, and `to_plain_text` serialize a copied `Slice` in the markup TipTap renders and in CommonMark with GitHub task lists and strikethrough.
- `clipboard::from_html`, `from_markdown`, and `from_plain_text` parse pasted content back into a schema-valid slice: unknown markup degrades to text, bare paragraphs inside lists gain list items, and Google Docs' inline `font-weight`, `font-style`, and `text-decoration` styles become marks.
- The parsers are small and hand-written, adding no dependencies and no HTML5 or CommonMark engine to WebAssembly builds.
- `canvist_editor`'s paste keeps the type of an open first block pasted into an empty block and keeps an explicit empty last block.

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)

#### Fit the web editor in its 350 KB budget

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` now ships 349.8 KB gzipped, measured as an app bundles it (`npm run size`, enforced in CI). The WebAssembly module is optimized without inlining, which gzips smaller.
- `canvist_layout::css_number` formats CSS numbers without `core`'s float formatting, which no longer links into the module.
- `EditorState::change` and `InputRule::new` keep one copy of their logic instead of one per command and rule, and `Engine::metadata` is built on first use.
