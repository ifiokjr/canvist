All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.1.1](https://github.com/canvist/canvist/releases/tag/v0.1.1) (2026-10-20)

### 🚀 Feature

#### Add `canvist_editor`, the platform-agnostic editing engine

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #90](https://github.com/canvist/canvist/pull/90)

Add `canvist_editor`, the platform-agnostic editing engine, and `Transaction::allocate_id` in `canvist_model` so commands can create blocks inside a transaction.

- `EditorState` holds the document, selection, stored marks, and IME composition. Every change goes through `EditorState::change` or `EditorState::apply_steps`, which map the selection and composition, clear stale stored marks, and keep the document non-empty.
- Commands as plain functions: typing with mark inclusivity, deleting by grapheme, word, line, or document (joining blocks and lifting out of containers at block boundaries), marks, select all, block selections, and IME composition.
- Selection motion by grapheme cluster and word (UAX #29), line, and document, treating embeds and atomic blocks as single stops. Vertical motion goes through the `LineNavigator` trait, with a paragraph-based fallback for headless use.
- An outline notation for before/after command tests.

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

#### Add `canvist_model`, the v2 block-based document model

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #86](https://github.com/canvist/canvist/pull/86)

- `Schema` built from block, mark, and embed specs that extensions contribute.
- `Document` with stable `BlockId`s, nested containers, and `InlineText` that stores formatting as mark runs and embeds as single characters.
- Invertible `Step`s with point mapping. `Transaction` applies them atomically with rollback, and `Commit` records which blocks changed for incremental layout.
- ProseMirror/TipTap-compatible JSON import and export.
- Property tests for invariants and exact undo, a 20,000-keystroke typing stress test, and `divan` benchmarks covering 100 to 100,000 blocks.

### 🐛 Fixed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
