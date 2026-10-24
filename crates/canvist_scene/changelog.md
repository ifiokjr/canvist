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

#### Add `canvist_scene`, the canvas's document model

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #128](https://github.com/canvist/canvist/pull/128)

`canvist_scene` models a canvas as a tree of typed nodes (pages, frames, groups, rectangles, ellipses, polygons and stars, vector paths, booleans, images, text frames, components, and instances), each with typed properties, ordered by fractional keys.

- Every change is one of five invertible steps (`InsertNodes`, `RemoveNodes`, `Move`, `SetProp`, `Text`), grouped in transactions that roll back on drop and invert exactly for undo.
- A text frame holds a full `canvist_model` document, so the text editor works inside it.
- Scenes read and write JSON in a canonical flat form and in a nested form, with optional ids and keys, that people and agents write.

### 🐛 Fixed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
