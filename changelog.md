# Changelog

## [0.1.1](https://github.com/canvist/canvist/releases/tag/v0.1.1) (2026-10-20)

Grouped release for `sdk`.

### 🚀 Feature

#### Load, transform, and export canvas scenes on a server

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist_headless_, 🟠 _canvist_headless_wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #136](https://github.com/canvist/canvist/pull/136)

`canvist_headless`'s new `scene` feature adds `SceneEngine` and `SceneDoc`. They load canvas scenes from JSON (flat or nested) or Yjs state, run canvas commands, edit inspector fields, transform scenes in transactions with undo, and export the page or chosen nodes as SVG or PNG with the renderer the canvas draws with. With `collab`, a scene shares and merges Yjs updates. Scene failures carry the existing stable error codes.

`@canvist/headless/scene` exposes the same in TypeScript, with `SceneJson` types for every node, property, paint, effect, and shape. It is a separate WebAssembly module, so the document entry point's size is unchanged.

#### Add `@canvist/headless`, the document model for JavaScript runtimes with no DOM

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist_headless_wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #110](https://github.com/canvist/canvist/pull/110)

Add `@canvist/headless` (`packages/canvist-headless`), canvist's document model for JavaScript runtimes with no DOM, over a new `canvist_headless_wasm` crate (wasm-bindgen, no `web-sys`).

- Loads in Node, Deno, and Bun from disk, and in Workers from an imported `WebAssembly.Module`. A `build` step emits JavaScript for npm, and CI installs the packed tarball as a dependency.
- Types come from the extension metadata (`src/extensions.generated.ts`, generated from the `canvist_extensions` snapshot): an engine's documents only offer the registered extensions' commands, queries, blocks, and marks, with argument types such as heading levels `1 | … | 6`. `tests/types.check.ts` pins this with `tsc --noEmit` and `@ts-expect-error`.
- The shared conformance fixtures pass in Node and Deno (30 of 30), alongside API tests that sync with JavaScript Yjs.
- `examples/server` is a Deno server that converts Markdown to JSON over HTTP and runs a y-websocket-compatible sync endpoint, tested end to end with real Yjs clients.
- `bench/overhead.ts` records per-call overhead and the wasm size against budgets.

#### Release the SDK to every registry from one release pull request

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #125](https://github.com/canvist/canvist/pull/125)

Releases move from knope to monochange. One version covers every SDK package, and the release pull request publishes each one:

- The Rust crates `canvist_model`, `canvist_editor`, `canvist_extensions`, `canvist_layout`, `canvist_collab`, and `canvist_headless` go to crates.io, and `@canvist/headless` goes to npm and JSR. All three registries use trusted publishing.
- The Android library goes to Maven Central as `io.github.ifiokjr:canvist-android`.
- The Swift package's `CanvistFFI.xcframework.zip`, its checksum, and `Canvist.swift` are attached to the GitHub release.
- The Dart packages are ready to reserve their pub.dev names. `canvist_flutter` now depends on `canvist: ^0.1.0` and resolves the local package through `pubspec_overrides.yaml`.

#### Add `@canvist/web`, the canvas editor for the browser, with a fonts plugin

_Packages:_ 🟠 _@canvist/headless_, 🟠 _@canvist/web_, 🟠 _canvist_headless_, 🟠 _canvist_layout_, 🟠 _canvist_web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` and `canvist_web` paint the document on an opaque Canvas2D context with HiDPI backing stores, dirty-rectangle repaints, and baselines and backgrounds snapped to the browser's own pixel grid. `createEditor` types commands, queries, and plugin APIs from what you register.
- Text is measured by the browser: `measureText` advances with kerning placed as shapers place it, line boxes read from the DOM, family lists resolved to the face the DOM uses, and DOM ranges for Arabic and for Firefox's fractional sizes. The canvas wraps where server-rendered HTML does.
- `@canvist/web/fonts` loads faces from URLs or bytes with `unicode-range` and `font-display`, builds metric-compatible fallbacks, lists local fonts through `queryLocalFonts`, and writes the same faces as `@font-face` CSS for SSR.
- `canvist_layout`: `TextMeasurer` batches carry line-box requests that a measurer may answer; `DocumentLayout::forget_fonts` relays out only the blocks a loaded face affects; `measure_stats`, `line_ranges`, `fonts`, and `font_metrics`; kerning between adjacent segments; `chromium_breaks`; border widths snapped to device pixels; `Theme::starter_kit_with` for dark and custom-font themes, and `Chrome::background`. `to_html` now sets the background and `text-spacing-trim: normal`, the StarterKit code stack no longer names `ui-monospace`, and graphemes come from the editor's segmenter. `ParleyMeasurer` sets a variable font's optical size to the font size, as browsers do.
- `canvist_headless`: `Doc::edit` runs editor calls with a host's clock and line navigator, and `Doc::command_named` resolves a named command.
- `@canvist/headless` exports its generated extension descriptors at `@canvist/headless/extensions`, with no WebAssembly.

#### React and Vue bindings, and hydrating server-rendered HTML

_Packages:_ 🟠 _@canvist/react_, 🟠 _@canvist/vue_, 🟠 _@canvist/web_, ⚪ _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #141](https://github.com/canvist/canvist/pull/141)

- `@canvist/react` adds `useCanvistEditor`, `useEditorState` (through `useSyncExternalStore`), and `<CanvistEditor />`. `@canvist/vue` adds the same composables and a component that emits `ready` and `update`. Both are typed from the extensions and plugins you register.
- `editor.subscribe(listener)` in `@canvist/web` calls back after every document or selection change.
- `createEditor` hydrates server-rendered HTML: given an element holding canvist's `.cv-doc` markup, it reads the document from it, keeps it on screen until fonts settle, and swaps in the canvas in one frame, with no layout shift.
- `examples/web` shows a plain page, React, Vue, and a page rendered by the new `ssr_page` example in `canvist_headless`, with Playwright tests in every browser.

#### Fit the web editor in its 350 KB budget

_Packages:_ 🟢 _@canvist/web_, 🟢 _canvist_editor_, 🟢 _canvist_headless_, 🟢 _canvist_web_, 🟠 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` now ships 349.8 KB gzipped, measured as an app bundles it (`npm run size`, enforced in CI). The WebAssembly module is optimized without inlining, which gzips smaller.
- `canvist_layout::css_number` formats CSS numbers without `core`'s float formatting, which no longer links into the module.
- `EditorState::change` and `InputRule::new` keep one copy of their logic instead of one per command and rule, and `Engine::metadata` is built on first use.

#### Edit on the web: EditContext input, IME, clipboard, drag and drop, accessibility

_Packages:_ 🟠 _@canvist/web_, 🟠 _canvist_headless_, 🟠 _canvist_web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #140](https://github.com/canvist/canvist/pull/140)

- Text arrives through `EditContext` in Chromium, with the paragraph and caret bounds the platform's input methods need, and through a hidden textarea everywhere else. `input: "textarea"` forces the textarea, as a kill switch for `EditContext` problems.
- Compositions from input methods and dead keys show as preedit text and commit in place. Key bindings follow the platform (`Mod` is Cmd on Apple devices), detected from the browser or set with `platform`.
- Click, Shift-click, double-click, and triple-click select; dragging extends by the unit the press selected. Copy, cut, and paste carry HTML and plain text; the selection can be dragged to move or copy it, and drops from other applications land where they fall.
- A visually hidden copy of the document, the same semantic HTML the server renders, is refreshed on idle for screen readers and find-in-page.
- Every input event is painted in its own task. `editor.inputLatency()` reports input-to-paint times; the browser tests hold their 95th percentile under 8 ms and type a 1,000-key burst without dropping a key.
- `Engine::with_platform` in `canvist_headless` chooses what `Mod` means in key bindings.

#### Add cursor-aware wasm editing and the `@canvist/canvist` TypeScript package

_Packages:_ 🟠 _canvist_v1_, 🟠 _canvist_core_, 🟠 _canvist_render_, 🟠 _canvist_wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #4](https://github.com/canvist/canvist/pull/4)

###### Features

- **canvist_wasm**: Added `insert_text_at(offset, text)` and `delete_range(start, end)` for cursor-aware editing
- **@canvist/canvist**: New Deno/JSR/npm package with TypeScript API wrapping the WASM editor
  - `createEditor(canvasId)` — high-level editor with keyboard input, cursor, backspace, delete, Enter, arrow keys
  - `initWasm()` — explicit WASM initialization
  - Full demo page with canvas editor and contenteditable comparison
  - Playwright tests passing on Chromium, Firefox, and WebKit (Safari)

###### Fixes

- **CI**: Fixed semver-checks to skip crates not yet published to crates.io
- **CI**: Added disk space cleanup step to prevent nix/devenv setup failures
- **CI**: Fixed clippy/lint to exclude wasm and test crates on native targets
- **CI**: Added Playwright CI workflow for cross-browser testing

#### Revive canvist as a canvas editor with a Rust core, wasm renderer, and CI

_Packages:_ 🟠 _canvist_v1_, 🟠 _canvist_core_, 🟠 _canvist_render_, 🟠 _canvist_wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #3](https://github.com/canvist/canvist/pull/3)

###### Features

- **canvist**: Umbrella re-export crate with `prelude` module and optional `wasm` feature
- **canvist_core**: Full document model with node tree (paragraphs, text runs), selection/cursor model, atomic edit operations with transaction batching, composable text styling with fluent builder API, Unicode-aware text layout with line breaking, and real-time collaboration via Yjs CRDTs
- **canvist_render**: Platform-agnostic `Canvas` and `Renderer` traits, `Viewport` with zoom/scroll/coordinate transforms, and `FontCache` via fontdue
- **canvist_wasm**: WebAssembly + Canvas2D rendering backend with `CanvistEditor` JS class, DOM event bridge, and accessibility shadow DOM

###### Infrastructure

- Updated Rust toolchain to 1.93.1 (edition 2024)
- Modernized devenv.nix/yaml with rust-overlay, ifiokjr-nixpkgs, and comprehensive dev scripts
- Added knope.toml for changeset-based releases matching mdt patterns
- Added cargo-deny, clippy, rustfmt, and dprint configurations
- Added CI workflows (lint, test, build, security, semver-checks) and release workflow
- Added Playwright-rs integration test infrastructure
- Added mdbook documentation with architecture guide, API overview, and contributing guide

#### Add `canvist`, a pure Dart package, and rebuild `canvist_flutter` on it

_Packages:_ 🟠 _canvist-dart_, 🟠 _canvist_flutter_, 🟠 _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #113](https://github.com/canvist/canvist/pull/113)

Add `canvist`, a pure Dart package (`platforms/dart`, no Flutter) over `canvist_headless`, and rebuild `canvist_flutter` on top of it.

- The Rust adapter (`canvist_dart`, formerly `canvist_flutter`) binds `canvist_headless` through flutter_rust_bridge and is compiled by the Dart build hook, so `dart run`, `dart test`, and `dart build cli` work on macOS and Linux with the Dart SDK alone. A small loader finds the library through the Dart VM's native-assets mapping, which flutter_rust_bridge's file-name loader cannot.
- `EngineBuilder.add(Extensions.bold)` returns typed handles, generated from the extension metadata, whose methods return typed `Command`s and `Query<T>`s. Engines, documents, formats, UTF-16 positions, snapshots, `CanvistException` with shared `ErrorCode`s, and the Yjs peer and y-sync server API mirror the Rust and TypeScript packages.
- `dart test` passes the shared conformance fixtures (30 of 30), API tests, and an end-to-end test of the `dart:io` example server (Markdown conversion plus a y-websocket endpoint) with two peers over WebSockets. A new `dart` workflow runs them on Linux and macOS without Flutter.
- `canvist_flutter` now depends on and re-exports `canvist`; its example app and integration tests use the real engine instead of the placeholder handle.
- `Engine::named` registers StarterKit extensions by name, shared by the TypeScript and Dart bindings.

#### Add `canvist_canvas`, the canvas's headless interaction core

_Packages:_ 🟠 _canvist_canvas_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #133](https://github.com/canvist/canvist/pull/133)

`canvist_canvas` turns pointer, wheel, and key input into scene edits, with no UI of its own. Hosts feed it input and draw the scene and its overlay.

- A camera with zoom about a point, momentum, and pinch; an R-tree spatial index; and exact hit testing by fill, stroke, and text layout.
- Selection, marquee, transform handles that keep scale out of the transform, and snapping with guides.
- Tools for selection, panning, shapes, frames, the pen, and text, registered as typed extensions alongside commands and inspector fields. Machine-readable metadata describes them for bindings and agents.
- Auto-layout, constraints, and text frames laid out with `canvist_layout`.

#### Render the canvas, and edit text frames with the real text editor

_Packages:_ 🟠 _canvist_canvas_, 🟠 _canvist_canvas_wasm_, 🟠 _canvist_editor_, 🟠 _canvist_paint_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #134](https://github.com/canvist/canvist/pull/134)

`canvist_paint` (new) draws a canvas page through one `Painter` contract. Backends:

- vello on wgpu: WebGPU in browsers, and Metal, Vulkan, and DX12 natively;
- `vello_cpu`, for PNG export and the browser fallback;
- SVG, with text kept as text.

The scene walker culls to the viewport and has level of detail. Text is shaped with parley and the bundled Atkinson Hyperlegible font, so a frame wraps the same on the canvas, in a PNG, and in SVG. With 10,000 nodes on screen, encoding a GPU frame takes about 1 ms and a 1080p CPU frame about 4 ms.

`canvist_canvas` adds `TextSession` (feature `editor`). It opens a text frame in a `canvist_editor::Editor`, places the caret where you click, draws the caret and selection in the overlay, and hands the document back on close. It also adds `Canvas::tool_named`.

`canvist_editor` adds `Editor::into_state`, so a host that lent a document to an editor can take it back.

`canvist_canvas_wasm` (new, unpublished) runs the canvas in the browser on WebGPU with a CPU fallback, behind the `@canvist/canvas` demo.

#### Collaborate on canvas scenes with `SceneBinding`

_Packages:_ 🟠 _canvist_collab_, 🟠 _canvist_scene_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #131](https://github.com/canvist/canvist/pull/131)

A new `scene` feature adds `SceneBinding`, which mirrors `canvist_scene` steps to a Yjs document and back with the same guarantees as the document binding: exact collaborative undo of your own changes, reordering of updates that arrive early, and convergence however updates interleave.

- Each node is a `Y.Map` in one flat `"nodes"` map. Its position is an atomic `[parent, key, clock, client, previousParent]` register, and each property is its own last-writer-wins entry.
- Text frames keep their text inside the scene's Yjs document, in exactly the document binding's shape, so creating a frame with its first paragraph is one atomic change.
- Orphans, unknown types, and nodes their parent rejects are hidden but kept. When concurrent moves form a cycle, the later move loses.
- The block binding's mirror is split from its Yjs document so both bindings share it. `Binding`'s API is unchanged.

`canvist_scene` gains `Transaction::text_external`, which applies a collaborator's text change to a text frame through the frame's own document, and `Prop::fits`, which checks a property value against a node kind.

#### Add `canvist_cloud`, starting with Sync: a multi-tenant y-sync relay

_Packages:_ 🟠 _canvist_collab_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #116](https://github.com/canvist/canvist/pull/116)

- Stock `y-websocket` and Hocuspocus clients connect to `/v1/sync/{document}` and `/v1/hocuspocus`, with presence, read-only sessions, and per-document access from short-lived HS256 tokens that API keys mint.
- Documents persist to Postgres as merged Yjs updates, compact into snapshots, and leave memory once idle. Updates that write outside the `blocks` root or put bare values in it are refused.
- Debounced, signed `document.updated` webhooks; REST endpoints to list, read, write, and delete documents; health and readiness probes; and a graceful shutdown that persists every room.
- `canvist_collab::Reorder` is now public, so hosts with a bare yrs document apply out-of-order updates safely.

#### Add `canvist_collab`, the Yjs collaboration binding for `canvist_model` documents

_Packages:_ 🟠 _canvist_collab_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #89](https://github.com/canvist/canvist/pull/89)

- `Binding` translates committed steps into yrs operations (UTF-8 offsets become UTF-16) and remote yrs events into steps, with no tree diffing. A keystroke costs about 2 µs each way, independent of document size.
- A documented Yjs schema (`Y.Map` blocks, `Y.Text` with marks as formatting and embeds as values, nested `Y.Array`s) that JavaScript Yjs peers read and write directly.
- Out-of-order and duplicate updates converge: updates wait until what they build on arrives, working around yrs 0.27–0.28 integrating them early.
- Blocks that break the schema are hidden, not deleted, and reappear once valid. Concurrent moves of one block keep the copy with the smallest yrs item id.
- Shared undo and redo through a yrs `UndoManager` scoped to local changes, with explicit step grouping.
- Property tests for 2- and 4-peer convergence under reordered and duplicated delivery, a Deno test against JavaScript `yjs`, measurements of concurrent-split behaviour, and `divan` benchmarks.

#### Add syncing, presence, and a swappable history trait to `canvist_collab`

_Packages:_ 🟠 _canvist_collab_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #96](https://github.com/canvist/canvist/pull/96)

- `SyncSession` speaks the y-sync protocol (sync step 1 and 2, updates, awareness) over any transport, framed for y-websocket servers or for Hocuspocus, including its authentication step.
- `Presence` publishes the local user and selection through Yjs awareness as relative positions, in y-prosemirror's state shape, and resolves other peers' carets into `Collaborator`s.
- `History` is the trait an editor uses for undo; `Binding` implements it with a yrs `UndoManager` that only undoes local changes.
- Tests sync through a yrs relay server and through a real Hocuspocus server with a JavaScript `HocuspocusProvider`, carets included.

#### Add `canvist_headless`, the document model and editing engine with no UI

_Packages:_ 🟠 _canvist_collab_, 🟠 _canvist_editor_, 🟠 _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #108](https://github.com/canvist/canvist/pull/108)

Add `canvist_headless`, the canvist document model and editing engine with no UI, for servers, scripts, and tests.

- `Engine` registers extensions once (with typed handles) and is `Send + Sync`; each `Doc` is `Send`. Load and export ProseMirror/TipTap JSON, HTML, Markdown, and plain text, run typed or named commands against explicit selections, answer queries, paste, undo, and take `Snapshot`s.
- `Position` addresses text by block path and UTF-16 offset, the same in every language. `ErrorCode` gives every failure a stable name.
- The `collab` feature makes a `Doc` a Yjs peer and a y-sync server endpoint (`receive_sync` returns the reply and what to relay), and `validate` reports shared content the schema rejects. The `ssr` feature adds a `Renderer` for HTML and SVG through `canvist_layout` and `parley` with a bundled font. The default build has neither.
- `conformance/` holds language-neutral fixtures (input, operation script, expected output) that every binding runs; the Rust runner passes all 30.
- `canvist_editor`: `Editor::apply_external` and `EditorState::apply_external` let a collaboration binding change the editor's own document, so a shared document is held once.
- `canvist_collab`: `Binding::rejected_blocks` counts shared items the schema hides, and the `sync` feature makes `Binding` and `SyncSession` `Send + Sync`.

#### Fix selection-scoped formatting, mouse interaction, and toolbar layout

_Packages:_ 🟠 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #52](https://github.com/canvist/canvist/pull/52)

###### Breaking changes to `Document::apply_style`

`apply_style()` now properly splits text runs at selection boundaries when applying formatting to a partial run. Previously it merged the style into the entire overlapping run, causing formatting to "leak" beyond the selected text.

###### Bug fixes

- **Formatting precision:** Selecting a portion of text and toggling bold/italic/underline now only affects the selected characters, not the entire text run. Runs are split into up to 3 pieces (before, selected, after) as needed.
- **Undo/redo:** `restore_run_styles()` now handles split runs by merging them back into the original run shape when undoing a format operation.
- **Double-click selection:** Double-clicking to select a word no longer loses the selection when the click event fires after mousedown.
- **Triple-click-and-drag:** Triple-clicking now enters line-level selection mode. Dragging after a triple-click extends the selection line-by-line (standard macOS behavior). Double-click-and-drag extends word-by-word.
- **Toolbar overflow:** The formatting toolbar now wraps to multiple rows with grouped controls instead of overflowing the viewport.
- **IME/composition:** Full `compositionstart`, `compositionupdate`, and `compositionend` event handling for CJK and other IME input methods.

#### Add Markdown import/paste and improved README

_Packages:_ 🟠 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #62](https://github.com/canvist/canvist/pull/62)

###### Markdown import

- `Document::from_markdown()` parses `**bold**`, `*italic*`, `~~strike~~`, and double-newline paragraph breaks into the document model.
- `parse_simple_markdown()` exposed as a public utility.
- `CanvistEditor::paste_markdown()` WASM method for pasting Markdown with formatting preserved.

###### README

Rewrote the `@canvist/canvist` package README with:

- Feature highlights (rendering, formatting, undo, collab, a11y, paste)
- Quick start example with formatting
- Collaboration code example
- API reference table
- Links to both demos (editor + collab)

#### Proper paragraph splitting and merging for multi-paragraph documents

_Packages:_ 🟠 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #57](https://github.com/canvist/canvist/pull/57)

###### Paragraph splitting on newline

`Document::insert_text()` now creates separate `Paragraph` nodes when the inserted text contains newlines, instead of embedding `\n` characters into a single text run. This gives the document tree proper semantic structure:

```text
Before: Root → Para → Run("hello\nworld")
After:  Root → Para → Run("hello")
             → Para → Run("world")
```

###### Paragraph merging on delete

`Document::delete()` now detects when a deletion crosses a paragraph boundary (spans a `\n` in the plain text) and merges the affected paragraphs by moving the second paragraph's runs into the first. Empty paragraphs are cleaned up automatically via `merge_empty_paragraphs()`.

###### Updated serialization

`to_html()` and `to_markdown()` now iterate the paragraph tree directly instead of splitting on `\n` within run text, producing correct multi-paragraph output.

###### Updated `paragraph_count()`

Now returns the actual number of `Paragraph` nodes in the tree (with a minimum of 1 for empty documents).

#### Add run compaction, collaboration wiring, and demo persistence

_Packages:_ 🟠 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #55](https://github.com/canvist/canvist/pull/55)

###### Run compaction

`Document::compact_runs()` is now called automatically during `rebuild_indexes()`. Adjacent text runs with identical styles are merged into a single run, preventing fragmentation from repeated format/undo cycles. Empty runs are also removed (except the last run in a paragraph, which is kept to preserve document structure for insert operations).

###### Collaboration WASM API

The `CanvistEditor` now exposes a complete collaboration API to JavaScript:

- `enable_collab()` — creates a Yrs CRDT session and syncs the document
- `collab_encode_state()` — full binary state for bootstrapping peers
- `collab_encode_state_vector()` — state vector for incremental sync
- `collab_apply_update(update)` — apply a remote peer's binary update
- `collab_sync_local()` — push local edits into the CRDT

###### Demo persistence

The demo now saves/restores document content and cursor position via `localStorage`. Content auto-saves every 2 seconds and on page unload.

#### Add undo coalescing and improved cursor positioning after undo/redo

_Packages:_ 🟠 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #54](https://github.com/canvist/canvist/pull/54)

###### Undo coalescing

Consecutive single-character inserts that arrive within a configurable timeout window (default 500ms) are now merged into a single undo group. This gives users a natural "word at a time" undo experience instead of undoing one character at a time.

- Newlines, multi-character inserts, deletions, and formatting operations all break the coalescing chain and start a new undo group.
- The timeout is configurable via `set_coalesce_timeout_ms()`.
- `Transaction::compose()` enables merging inverse transactions.

###### Cursor positioning

Undo and redo now restore the cursor to a meaningful position instead of jumping to the end of the document:

- After undoing an insert, the cursor is placed at the insert position.
- After undoing a delete, the cursor is placed after the restored text.
- `Transaction::cursor_hint()` computes the appropriate position from the transaction's operations.

#### Add clipboard formats to `canvist_extensions` behind the `clipboard` feature

_Packages:_ 🟢 _canvist_editor_, 🟠 _canvist_extensions_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #94](https://github.com/canvist/canvist/pull/94)

Add clipboard formats to `canvist_extensions` behind the `clipboard` feature (part of `starter-kit`).

- `clipboard::to_html`, `to_markdown`, and `to_plain_text` serialize a copied `Slice` in the markup TipTap renders and in CommonMark with GitHub task lists and strikethrough.
- `clipboard::from_html`, `from_markdown`, and `from_plain_text` parse pasted content back into a schema-valid slice: unknown markup degrades to text, bare paragraphs inside lists gain list items, and Google Docs' inline `font-weight`, `font-style`, and `text-decoration` styles become marks.
- The parsers are small and hand-written, adding no dependencies and no HTML5 or CommonMark engine to WebAssembly builds.
- `canvist_editor`'s paste keeps the type of an open first block pasted into an empty block and keeps an explicit empty last block.

#### Add `canvist_editor`, the platform-agnostic editing engine

_Packages:_ 🟠 _canvist_editor_, 🟠 _canvist_model_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #90](https://github.com/canvist/canvist/pull/90)

Add `canvist_editor`, the platform-agnostic editing engine, and `Transaction::allocate_id` in `canvist_model` so commands can create blocks inside a transaction.

- `EditorState` holds the document, selection, stored marks, and IME composition. Every change goes through `EditorState::change` or `EditorState::apply_steps`, which map the selection and composition, clear stale stored marks, and keep the document non-empty.
- Commands as plain functions: typing with mark inclusivity, deleting by grapheme, word, line, or document (joining blocks and lifting out of containers at block boundaries), marks, select all, block selections, and IME composition.
- Selection motion by grapheme cluster and word (UAX #29), line, and document, treating embeds and atomic blocks as single stops. Vertical motion goes through the `LineNavigator` trait, with a paragraph-based fallback for headless use.
- An outline notation for before/after command tests.

#### Add the `Editor`: history, keymaps, input and paste rules, and the `Extension` trait

_Packages:_ 🟠 _canvist_editor_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #92](https://github.com/canvist/canvist/pull/92)

- `Editor` and `EditorBuilder` assemble the schema, commands, key bindings, and rules from extensions, sorted by priority, and route every update through one place that runs extension fix-ups and records history.
- `History` is the boundary the collaboration crate replaces; `UndoHistory` groups typing (including IME composition) and deletions by time (500 ms by default), keeps other edits as their own steps, restores the selection, and limits depth.
- `Keymap` parses ProseMirror-style bindings such as `Mod-Shift-z`, with `Mod` meaning Cmd on Apple platforms and Ctrl elsewhere. `BaseKeymap` binds each platform's standard caret, deletion, Enter, select-all, and Escape keys.
- `InputRule` rewrites text as it is typed (block type, wrapping, marks, text replacement, and atomic blocks), runs only on whitespace or punctuation, and skips code blocks. Backspace right after a rule restores the typed text. `PasteRule` formats pasted text, for example into links.

#### Settle the `LineNavigator` interface that `canvist_layout` implements

_Packages:_ 🟠 _canvist_editor_, 🟢 _canvist_extensions_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #103](https://github.com/canvist/canvist/pull/103)

- `LineNavigator` now has `vertical(&mut self, doc, from: Caret, direction: VerticalDirection, goal_x: Option<f32>) -> Result<Option<VerticalMotion>, NavigationError>` and `line_edge(&mut self, doc, at: Caret, edge: LineEdge) -> Result<Caret, NavigationError>`. Methods take `&mut self` so layout can measure blocks lazily, and return errors instead of guessing when it cannot.
- `Caret` pairs a point with an `Affinity` (`Upstream` or `Downstream`), so the offset where a line soft-wraps can be the end of one line or the start of the next. `EditorState::affinity` tracks it: line and vertical motion set it, and every other change resets it to `Downstream`.
- `Env` holds the navigator mutably. Build one with `Env::new(now, &mut navigator)`, or `Env::headless(now)` for the `ParagraphNavigator` fallback. `Editor` methods now take `&mut Env`.
- `EditorError::Navigation` carries a navigator's error.

#### Add structural editing commands to `canvist_editor`

_Packages:_ 🟠 _canvist_editor_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #91](https://github.com/canvist/canvist/pull/91)

- Enter splits text blocks (a heading continues as a paragraph at its end), and Backspace and Delete join blocks across containers, adopting the rest of a joined list item or quote.
- Block types, block attributes, wrapping and lifting (such as blockquotes), and atomic blocks such as horizontal rules.
- Lists generic over list and item types: split, indent, outdent, join with the previous item, wrap, and toggle, converting between list types in place while keeping block ids.
- `Slice` for copy, cut, and paste: copying keeps partly selected containers around their children, and pasting merges open text at both ends and converts or flattens blocks the destination does not accept.
- Property tests that random command sequences keep every invariant and that every update's inverse restores the document exactly.

#### Add `canvist_extensions`, the StarterKit, with one Cargo feature per extension

_Packages:_ 🟠 _canvist_editor_, 🟠 _canvist_extensions_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #93](https://github.com/canvist/canvist/pull/93)

Add `canvist_extensions`, the StarterKit, with each extension behind its own Cargo feature.

- Nodes: `Document`, `Paragraph`, `Heading`, `Blockquote`, `CodeBlock`, `HardBreak`, `HorizontalRule`, `BulletList`, `OrderedList`, `ListItem`, and `TaskList`, with TipTap's names, key bindings, and Markdown-style input rules.
- Marks: `Bold`, `Italic`, `Underline`, `Strike`, `Code`, and a non-inclusive `Link` with autolinking and linking on paste.
- Functionality: `History` keys and `TrailingNode`.
- `canvist_editor` gains `UndoInputRule` (Backspace takes back an input rule before any other Backspace binding), `lift_empty_block`, and a public `wrap_in_list`, and its hot paths avoid scanning sibling lists so a keystroke costs the same at 100 or 100,000 blocks.
- A scripted-typing harness (`"## Hi⏎- one⏎two⇥"`) with JSON snapshots, property tests that random keystroke scripts keep every invariant and undo and redo exactly, and `divan` keystroke benchmarks at 100, 10,000, and 100,000 blocks.

#### Add the headless, typed extension API (architecture Decision 8)

_Packages:_ 🟠 _canvist_editor_, 🟠 _canvist_extensions_, 🟠 _canvist_model_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #102](https://github.com/canvist/canvist/pull/102)

- `canvist_model`: `Document::savepoint`, `rollback_to`, and `release`. A rollback restores the content, the version counter, every block revision, and id allocation exactly, so a dry run leaves layout caches warm. Savepoints nest.
- `canvist_editor`:
  - `EditorBuilder::add` returns an extension's typed handle, behind `TypedExtension` and an unforgeable `Registration`.
  - `CommandSpec` and `QuerySpec` declare named commands and queries with typed, checked arguments (`Field`, `ValueType`). `Editor::run_named`, `command_named`, and `query` serve dynamic hosts, and `Editor::metadata` and `metadata_json` describe every node, attribute, command, and query for binding generators.
  - `Editor::can` and `EditorState::dry_run` answer whether a command would apply without changing anything. `Editor::chain` runs commands all or nothing as one undo step, and `EditorState::atomic` backs it.
  - `Extension::commands` now returns `CommandSpec`s, and `queries` and `nodes` are new.
  - Input rules run only when the typed character can complete their pattern, and unanchored rules search a bounded `Lookback` (64 bytes by default, or the current word for autolinks). A keystroke at the end of a long line no longer scans 500 bytes per rule.
- `canvist_extensions`: every extension has a typed handle (`HeadingHandle::toggle(HeadingLevel::H2)`, `MarkHandle::is_active`, `LinkHandle::href`, ...), argument-checked commands, queries, and node metadata. `StarterKit::add_all` returns all the handles.

#### Add the native build pipeline for iOS, Android and Flutter

_Packages:_ 🟠 _canvist_ffi_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #87](https://github.com/canvist/canvist/pull/87)

- New `canvist_ffi` crate exports a placeholder `EditorHandle` (`new`, `insert_text`, `text`, `version`) through UniFFI. It is backed by a plain `String` until the v2 engine replaces it.
- `platforms/apple`: `CanvistFFI.xcframework` (device + simulator arm64), generated Swift bindings in a `Canvist` Swift package, a SwiftUI sample app, and XCTest/XCUITest suites that run on an iOS Simulator.
- `platforms/android`: cargo-ndk builds for `arm64-v8a` and `x86_64`, generated Kotlin bindings in a `:canvist` Android library, a Compose sample app, and instrumented tests that run on an emulator.
- `platforms/flutter`: a `canvist_flutter` Dart package that binds the handle through flutter_rust_bridge v2 and builds Rust with Dart native-assets build hooks, plus an example app whose `integration_test` runs on both simulators.
- New devenv scripts `build:ios`, `build:android`, `test:ios`, `test:android` and `test:flutter`, a path-gated `native` CI workflow, and a native toolchain guide in the docs.

#### Add `canvist_layout`, the v2 layout engine

_Packages:_ 🟠 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #100](https://github.com/canvist/canvist/pull/100)

- Data-driven `Theme` rules for blocks, marks, and embeds, with a default matching TipTap's StarterKit.
- A batched `TextMeasurer` trait with a per-(font, segment) cache, a deterministic `FixedMeasurer`, and a `ParleyMeasurer` behind the `parley` feature.
- UAX #14 line breaking with CSS `pre-wrap` semantics, UAX #9 bidi, tabs, embeds, and hard breaks.
- `DocumentLayout` with container blocks, list markers, viewport virtualization with estimated heights, and scroll anchoring.
- Incremental relayout from a `Commit`. A keystroke re-breaks only the lines around the edit, even in a 100 KB paragraph, and a structural edit re-flattens only the root subtrees it touched.
- Hit testing, caret and selection rectangles, selection bounds for anchoring host UI, and a `LayoutNavigator` for vertical caret motion that implements `canvist_editor::LineNavigator` behind the `editor` feature.
- A `DisplayList` with dirty-rectangle damage, plus `to_svg` and `to_html` renderers whose CSS reproduces the canvas line breaks.
- Release-mode performance budget tests and `divan` benchmarks at 100, 10k, and 100k blocks, plus long and heavily formatted paragraphs.

#### Add `canvist_model`, the v2 block-based document model

_Packages:_ 🟠 _canvist_model_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #86](https://github.com/canvist/canvist/pull/86)

- `Schema` built from block, mark, and embed specs that extensions contribute.
- `Document` with stable `BlockId`s, nested containers, and `InlineText` that stores formatting as mark runs and embeds as single characters.
- Invertible `Step`s with point mapping. `Transaction` applies them atomically with rollback, and `Commit` records which blocks changed for incremental layout.
- ProseMirror/TipTap-compatible JSON import and export.
- Property tests for invariants and exact undo, a 20,000-keystroke typing stress test, and `divan` benchmarks covering 100 to 100,000 blocks.

#### Add `canvist_scene`, the canvas's document model

_Packages:_ 🟠 _canvist_scene_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #128](https://github.com/canvist/canvist/pull/128)

`canvist_scene` models a canvas as a tree of typed nodes (pages, frames, groups, rectangles, ellipses, polygons and stars, vector paths, booleans, images, text frames, components, and instances), each with typed properties, ordered by fractional keys.

- Every change is one of five invertible steps (`InsertNodes`, `RemoveNodes`, `Move`, `SetProp`, `Text`), grouped in transactions that roll back on drop and invert exactly for undo.
- A text frame holds a full `canvist_model` document, so the text editor works inside it.
- Scenes read and write JSON in a canonical flat form and in a nested form, with optional ids and keys, that people and agents write.

### 🐛 Fixed

- **Read the bundled WebAssembly module from a file path.** `load()` converts the bundled module's `file:` URL to a path before reading it, so the package type-checks under JSR's `deno publish` as well as with `@types/node`. _Packages:_ 🟢 _@canvist/headless_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #125](https://github.com/canvist/canvist/pull/125)
- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Packages:_ 🟢 _@canvist/headless_, 🟢 _@canvist/react_, 🟢 _@canvist/vue_, 🟢 _@canvist/web_, 🟢 _canvist_v1_, 🟢 _canvist_canvas_, 🟢 _canvist_canvas_wasm_, 🟢 _canvist_collab_, 🟢 _canvist_core_, 🟢 _canvist_editor_, 🟢 _canvist_extensions_, 🟢 _canvist_ffi_, 🟢 _canvist_layout_, 🟢 _canvist_model_, 🟢 _canvist_paint_, 🟢 _canvist_render_, 🟢 _canvist_scene_, 🟢 _canvist_wasm_, 🟢 _canvist_web_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)

#### Fix three ways collaborating peers' documents could diverge

_Packages:_ 🟢 _canvist_collab_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #135](https://github.com/canvist/canvist/pull/135)

- A block copy that loses its id to a concurrently moved copy is now evicted from where it sits in the shared array, not from its old place in the model. Blocks inserted after it in the same update used to land out of order on the receiving peer.
- A hidden copy of a block, such as one inside a list that no longer allows it, no longer claims the block's id when reading a document from scratch. `Binding::document` used to hide the shown copy and delete it from the shared document.
- `Reorder::take_ready` is replaced by `Reorder::take_next`, which releases one update at a time against the document's current state vector. An update that yrs keeps pending, because it is inserted next to another client's item that has not arrived, no longer lets that client's later updates overtake it. The binding and canvist Sync rooms both apply updates this way.

#### Clean up all clippy warnings across the workspace

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #65](https://github.com/canvist/canvist/pull/65)

Applied `cargo clippy --fix` and manually resolved remaining warnings:

- Inlined format arguments throughout test code
- Used `is_none_or` instead of `map_or(true, ...)` for option comparisons
- Replaced `unwrap_or_else` with `unwrap_or` for constant defaults
- Fixed loop variable indexing with iterator-based patterns
- Added `#[allow]` for intentional patterns (struct bools, single-char params, doc markdown)
- Removed unused dead code attributes
- Added underscore separators to long hex literals (FNV-1a constants)
- Simplified comparisons (`i < sel_end` instead of `i + 1 <= sel_end`)

#### Add collaboration demo, Ctrl+S save, and convergence tests

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #61](https://github.com/canvist/canvist/pull/61)

###### Collaboration demo

New `demo/collab.html` page demonstrates real-time collaborative editing between browser tabs using the Yrs CRDT + BroadcastChannel (no server needed). Each tab gets a random color cursor visible to other tabs. Edits sync automatically with debouncing.

###### Ctrl+S save

The main demo now handles Ctrl+S to save immediately to localStorage with a visual status bar flash and screen reader announcement.

###### New tests

5 new tests:

- **Delete across multiple paragraph boundaries** — removes 2 paragraphs at once
- **Insert at paragraph boundary** — inserts text at the start of a paragraph
- **Empty paragraphs from consecutive newlines** — verifies `A\n\nB` creates 3 paragraphs
- **Concurrent edit convergence** — 2 peers make concurrent edits, both converge
- **Three-peer convergence** — 3 peers with concurrent edits all converge

#### Improve crate READMEs and add license files for publishing

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #69](https://github.com/canvist/canvist/pull/69)

###### crate READMEs

- `canvist_core/readme.md` — expanded with features list, quick start example (runtime + formatting + undo), collaboration example, and export examples.
- `canvist_render/readme.md` — expanded with trait overview, backend implementation example, and component list.

###### Publishing readiness

- Added `license` file to each crate directory for crate packaging.
- Updated workspace `include` to include `license` in published packages.

#### Add doc tests for core public API

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #66](https://github.com/canvist/canvist/pull/66)

Added runnable doc examples to key public methods:

- `Document::insert_text` — basic insertion and paragraph splitting
- `Document::delete` — range deletion
- `Document::apply_style` — partial-run formatting with run splitting
- `Document::from_markdown` — Markdown import with styled runs
- `EditorRuntime::new` — runtime creation, text insert, and undo
- `Operation::insert` — create and apply an insert operation
- `Operation::delete` — create and apply a delete operation
- `Operation::format` — create and apply a format operation
- `CollaborationSession` — multi-peer sync example

Doc test count: 9 → 18 (+9 new).

#### Dynamic line height for mixed font sizes

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #63](https://github.com/canvist/canvist/pull/63)

The layout engine now computes line height dynamically from the tallest styled run on each line, instead of using the default style's font size for all lines.

A line containing both 12px and 48px text will now have a height of 72px (48px × 1.5 line-height multiplier) instead of 24px (16px × 1.5). This prevents large text from overlapping adjacent lines.

#### Add edge-case tests for paragraphs, serialization, and cross-boundary formatting

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #60](https://github.com/canvist/canvist/pull/60)

6 new tests covering critical edge cases:

- **Styled paragraph split:** Inserting a newline inside a bold run preserves bold on both resulting paragraphs.
- **JSON roundtrip:** Multi-paragraph document with formatting survives `to_json()` → `from_json()` with paragraph count and styles preserved.
- **HTML roundtrip:** Multi-paragraph styled document survives `to_html()` → `from_html()` with paragraph breaks and inline formatting preserved.
- **Collaboration sync:** Multi-paragraph document roundtrips through `sync_from_document()` → `sync_to_document()` preserving all paragraphs.
- **Delete entire paragraph:** Deleting a paragraph's content plus its trailing newline correctly merges remaining paragraphs.
- **Format across boundary:** Applying bold across a paragraph boundary (`\n`) correctly styles runs in both paragraphs.

#### Fix TypeScript type checking — 586 errors → 0

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #68](https://github.com/canvist/canvist/pull/68)

###### Root cause

The `WasmCanvistEditor` interface in `wasm.ts` only declared ~30 of the 568+ WASM methods. The `editor.ts` wrapper used all 568, causing 586 type errors that were masked by `--no-check` builds.

###### Fix

- `WasmCanvistEditor` now re-exports the auto-generated type from `wasm/canvist_wasm.d.ts` via `import type`, ensuring all WASM methods are available with correct signatures.
- Added `diff_texts` static method to `CanvistWasmModule` interface.
- Fixed `colorRgba` argument type: `number[]` → `Uint8Array`.
- Added missing `setTextAlign`, `textAlign`, `enableCollab`, `collabEnabled`, `collabEncodeState/Vector`, `collabApplyUpdate`, `collabSyncLocal` to the public editor API object.

`deno check src/editor.ts` now passes with **zero errors**.

#### Add pending style for collapsed-cursor formatting and clean up WASM warnings

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #56](https://github.com/canvist/canvist/pull/56)

###### Pending style

Toggling bold, italic, underline, or strikethrough with a collapsed cursor (no selection) now sets a "pending style" that is automatically applied to the next text insertion. This matches the behavior of Google Docs and other rich-text editors where you can press Ctrl+B, type a word, and have it appear bold.

- `toggle_bold/italic/underline/strikethrough()` set `pending_style` when cursor is collapsed.
- `is_bold/italic/underline()` reflect the pending state for toolbar feedback.
- `insert_text_at()` consumes the pending style and formats the new text.
- Moving the cursor or changing selection clears the pending style.

###### Warning cleanup

All compiler warnings in `canvist_wasm` have been resolved:

- Removed unused `offset` variables in bracket auto-close and snippet expand.
- Added `#[allow(dead_code)]` to `LayoutConstants::new/with_zoom`.
- Added module-level `#![allow(dead_code)]` to `dom.rs` (future-use scaffolding).
- Prefixed unused `chars` variable in regex search.

#### Remove all unwrap()/expect() calls from production code

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #64](https://github.com/canvist/canvist/pull/64)

Eliminated every `unwrap()` and `expect()` call from all three production crates (`canvist_core`, `canvist_render`, `canvist_wasm`), replacing them with safe alternatives:

- `Option::map_or` for conditional best-span comparisons
- `let Some(...) = ... else { return }` for early returns
- Direct indexing with safety comments where emptiness is already checked
- `unwrap_or(0.0)` for optional numeric values

This ensures no panic paths exist in production code. Test code retains `unwrap()` for clarity since panics are the desired behavior for assertions.

#### Rich-text clipboard paste and integration tests

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #58](https://github.com/canvist/canvist/pull/58)

###### Rich-text paste

The demo's Ctrl+V handler now attempts to read HTML from the clipboard via the Clipboard API (`clipboard.read()` with `text/html` MIME type) before falling back to plain text. When HTML is available, it's passed to `paste_html()` which preserves bold, italic, underline, and strikethrough formatting from the source.

###### Integration tests

Added 3 new runtime integration tests:

- `format_partial_then_undo_preserves_text` — format a substring, undo, verify all runs are unstyled and text is preserved
- `insert_newline_creates_paragraph_then_undo_merges` — verify paragraph splitting and merging through undo
- `multiple_operations_undo_redo_roundtrip` — insert → format → insert → undo chain with redo verification

#### Add text alignment support and accessibility DOM

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #53](https://github.com/canvist/canvist/pull/53)

###### Features

- **Text alignment:** Added `set_text_align()` and `text_align()` methods to `CanvistEditor`. Supports left, center, right, and justify alignment. The layout engine's `x_offset` computation now applies the editor's alignment setting. Toolbar alignment buttons (⇤ ⇔ ⇥) are now functional with visual feedback showing the active alignment.

- **Accessibility DOM:** Added a hidden `role="document"` container with `aria-live="polite"` that mirrors the editor content as `<p>` elements for screen readers. This follows the Google Docs pattern of maintaining a parallel DOM for assistive technology while rendering to canvas.

- **Screen reader announcements:** Formatting actions (bold, italic, underline, strikethrough) now announce state changes via an `aria-live="assertive"` status region.

#### Fix TypeScript editor wrapper and update types

_Packages:_ 🟢 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #59](https://github.com/canvist/canvist/pull/59)

###### TypeScript editor fixes

Port the same mouse interaction fixes from the demo HTML to the TypeScript editor wrapper (`packages/canvist/src/editor.ts`):

- **Double-click selection persistence** via `skipNextClick` flag
- **Triple-click-and-drag** line-level selection with `selectionMode` tracking
- **Double-click-and-drag** word-level selection with word boundary snapping
- Added `wordAnchorStart/End` and `lineAnchorStart/End` for drag anchors

###### Updated TypeScript types

Added type definitions for new WASM APIs:

- `setTextAlign(align)` / `textAlign` — text alignment control
- `enableCollab()` / `collabEnabled` — collaboration session lifecycle
- `collabEncodeState()` / `collabEncodeStateVector()` — state export
- `collabApplyUpdate(update)` — remote update ingestion
- `collabSyncLocal()` — push local edits to CRDT

- **Build against yrs 0.28, which needs Rust 1.95 or newer.** The pinned toolchain moves to Rust 1.96. _Packages:_ 🟢 _canvist_core_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #88](https://github.com/canvist/canvist/pull/88)

<details>
<summary><strong>📖 Documentation</strong></summary>

#### Rewrite the docs for people and agents using canvist

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #130](https://github.com/canvist/canvist/pull/130)

- New structure: get started (one quick start per platform, each marked available, preview, or coming soon), core concepts, an extension catalogue, a build-your-own-editor guide, guides, an AI agent guide, canvist Cloud, and reference.
- Every Rust, TypeScript, and Dart sample runs in CI: Rust pages are doc tests of `canvist_book`, and `docs/check-snippets.ts` type-checks, analyzes, and runs the others, checking printed output against the comments.
- The website serves `/llms.txt` and `/llms-full.txt`, and groups the docs sidebar by `SUMMARY.md` parts. A draft `canvist` agent skill lives in `skills/canvist/`.
- Design records, research, the roadmap, and canvist Cloud operations move to `docs/internal/`, outside the published docs.

</details>

<details>
<summary><strong>🧪 Testing</strong></summary>

#### Test canvas and server-rendered HTML for pixel parity in every browser

_Packages:_ ⚪ _@canvist/web_, ⚪ _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #138](https://github.com/canvist/canvist/pull/138)

- Playwright renders a fixture matrix (system and web fonts, a variable font, CJK with `unicode-range` subsets, Arabic and Hebrew, Devanagari, three emoji formats, letter spacing, sizes from 10 to 64 px) as the canvas, the SSR HTML, and SVG, at device pixel ratios 1 to 3, fractional zoom, and light and dark themes, in Chromium, Firefox, and WebKit on macOS and Linux.
- Line boxes, runs, and caret stops are compared exactly; pixels with pixelmatch and SSIM against per-browser limits, with every known difference documented. Each run uploads a side-by-side report.
- A Rust test holds `parley`, with the same Inter file, to Chromium's line breaks.

</details>
