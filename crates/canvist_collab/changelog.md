All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Collaborate on canvas scenes with `SceneBinding`

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #131](https://github.com/canvist/canvist/pull/131)

A new `scene` feature adds `SceneBinding`, which mirrors `canvist_scene` steps to a Yjs document and back with the same guarantees as the document binding: exact collaborative undo of your own changes, reordering of updates that arrive early, and convergence however updates interleave.

- Each node is a `Y.Map` in one flat `"nodes"` map. Its position is an atomic `[parent, key, clock, client, previousParent]` register, and each property is its own last-writer-wins entry.
- Text frames keep their text inside the scene's Yjs document, in exactly the document binding's shape, so creating a frame with its first paragraph is one atomic change.
- Orphans, unknown types, and nodes their parent rejects are hidden but kept. When concurrent moves form a cycle, the later move loses.
- The block binding's mirror is split from its Yjs document so both bindings share it. `Binding`'s API is unchanged.

`canvist_scene` gains `Transaction::text_external`, which applies a collaborator's text change to a text frame through the frame's own document, and `Prop::fits`, which checks a property value against a node kind.

#### Add `canvist_cloud`, starting with Sync: a multi-tenant y-sync relay

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #116](https://github.com/canvist/canvist/pull/116)

- Stock `y-websocket` and Hocuspocus clients connect to `/v1/sync/{document}` and `/v1/hocuspocus`, with presence, read-only sessions, and per-document access from short-lived HS256 tokens that API keys mint.
- Documents persist to Postgres as merged Yjs updates, compact into snapshots, and leave memory once idle. Updates that write outside the `blocks` root or put bare values in it are refused.
- Debounced, signed `document.updated` webhooks; REST endpoints to list, read, write, and delete documents; health and readiness probes; and a graceful shutdown that persists every room.
- `canvist_collab::Reorder` is now public, so hosts with a bare yrs document apply out-of-order updates safely.

#### Add `canvist_collab`, the Yjs collaboration binding for `canvist_model` documents

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #89](https://github.com/canvist/canvist/pull/89)

- `Binding` translates committed steps into yrs operations (UTF-8 offsets become UTF-16) and remote yrs events into steps, with no tree diffing. A keystroke costs about 2 µs each way, independent of document size.
- A documented Yjs schema (`Y.Map` blocks, `Y.Text` with marks as formatting and embeds as values, nested `Y.Array`s) that JavaScript Yjs peers read and write directly.
- Out-of-order and duplicate updates converge: updates wait until what they build on arrives, working around yrs 0.27–0.28 integrating them early.
- Blocks that break the schema are hidden, not deleted, and reappear once valid. Concurrent moves of one block keep the copy with the smallest yrs item id.
- Shared undo and redo through a yrs `UndoManager` scoped to local changes, with explicit step grouping.
- Property tests for 2- and 4-peer convergence under reordered and duplicated delivery, a Deno test against JavaScript `yjs`, measurements of concurrent-split behaviour, and `divan` benchmarks.

#### Add syncing, presence, and a swappable history trait to `canvist_collab`

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #96](https://github.com/canvist/canvist/pull/96)

- `SyncSession` speaks the y-sync protocol (sync step 1 and 2, updates, awareness) over any transport, framed for y-websocket servers or for Hocuspocus, including its authentication step.
- `Presence` publishes the local user and selection through Yjs awareness as relative positions, in y-prosemirror's state shape, and resolves other peers' carets into `Collaborator`s.
- `History` is the trait an editor uses for undo; `Binding` implements it with a yrs `UndoManager` that only undoes local changes.
- Tests sync through a yrs relay server and through a real Hocuspocus server with a JavaScript `HocuspocusProvider`, carets included.

#### Add `canvist_headless`, the document model and editing engine with no UI

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #108](https://github.com/canvist/canvist/pull/108)

Add `canvist_headless`, the canvist document model and editing engine with no UI, for servers, scripts, and tests.

- `Engine` registers extensions once (with typed handles) and is `Send + Sync`; each `Doc` is `Send`. Load and export ProseMirror/TipTap JSON, HTML, Markdown, and plain text, run typed or named commands against explicit selections, answer queries, paste, undo, and take `Snapshot`s.
- `Position` addresses text by block path and UTF-16 offset, the same in every language. `ErrorCode` gives every failure a stable name.
- The `collab` feature makes a `Doc` a Yjs peer and a y-sync server endpoint (`receive_sync` returns the reply and what to relay), and `validate` reports shared content the schema rejects. The `ssr` feature adds a `Renderer` for HTML and SVG through `canvist_layout` and `parley` with a bundled font. The default build has neither.
- `conformance/` holds language-neutral fixtures (input, operation script, expected output) that every binding runs; the Rust runner passes all 30.
- `canvist_core`: `Editor::apply_external` and `EditorState::apply_external` let a collaboration binding change the editor's own document, so a shared document is held once.
- `canvist_collab`: `Binding::rejected_blocks` counts shared items the schema hides, and the `sync` feature makes `Binding` and `SyncSession` `Send + Sync`.

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

### 🐛 Fixed

#### Fix three ways collaborating peers' documents could diverge

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #135](https://github.com/canvist/canvist/pull/135)

- A block copy that loses its id to a concurrently moved copy is now evicted from where it sits in the shared array, not from its old place in the model. Blocks inserted after it in the same update used to land out of order on the receiving peer.
- A hidden copy of a block, such as one inside a list that no longer allows it, no longer claims the block's id when reading a document from scratch. `Binding::document` used to hide the shown copy and delete it from the shared document.
- `Reorder::take_ready` is replaced by `Reorder::take_next`, which releases one update at a time against the document's current state vector. An update that yrs keeps pending, because it is inserted next to another client's item that has not arrived, no longer lets that client's later updates overtake it. The binding and canvist Sync rooms both apply updates this way.

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
