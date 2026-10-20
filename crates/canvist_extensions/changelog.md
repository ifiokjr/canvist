All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.1.1](https://github.com/canvist/canvist/releases/tag/v0.1.1) (2026-10-20)

### 🚀 Feature

#### Add clipboard formats to `canvist_extensions` behind the `clipboard` feature

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #94](https://github.com/canvist/canvist/pull/94)

Add clipboard formats to `canvist_extensions` behind the `clipboard` feature (part of `starter-kit`).

- `clipboard::to_html`, `to_markdown`, and `to_plain_text` serialize a copied `Slice` in the markup TipTap renders and in CommonMark with GitHub task lists and strikethrough.
- `clipboard::from_html`, `from_markdown`, and `from_plain_text` parse pasted content back into a schema-valid slice: unknown markup degrades to text, bare paragraphs inside lists gain list items, and Google Docs' inline `font-weight`, `font-style`, and `text-decoration` styles become marks.
- The parsers are small and hand-written, adding no dependencies and no HTML5 or CommonMark engine to WebAssembly builds.
- `canvist_editor`'s paste keeps the type of an open first block pasted into an empty block and keeps an explicit empty last block.

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

#### Settle the `LineNavigator` interface that `canvist_layout` implements

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #103](https://github.com/canvist/canvist/pull/103)

- `LineNavigator` now has `vertical(&mut self, doc, from: Caret, direction: VerticalDirection, goal_x: Option<f32>) -> Result<Option<VerticalMotion>, NavigationError>` and `line_edge(&mut self, doc, at: Caret, edge: LineEdge) -> Result<Caret, NavigationError>`. Methods take `&mut self` so layout can measure blocks lazily, and return errors instead of guessing when it cannot.
- `Caret` pairs a point with an `Affinity` (`Upstream` or `Downstream`), so the offset where a line soft-wraps can be the end of one line or the start of the next. `EditorState::affinity` tracks it: line and vertical motion set it, and every other change resets it to `Downstream`.
- `Env` holds the navigator mutably. Build one with `Env::new(now, &mut navigator)`, or `Env::headless(now)` for the `ParagraphNavigator` fallback. `Editor` methods now take `&mut Env`.
- `EditorError::Navigation` carries a navigator's error.

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
