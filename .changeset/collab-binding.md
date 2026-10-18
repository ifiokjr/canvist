---
"canvist_collab": minor
---

Add `canvist_collab`, the Yjs collaboration binding for `canvist_model` documents.

- `Binding` translates committed steps into yrs operations (UTF-8 offsets become UTF-16) and remote yrs events into steps, with no tree diffing. A keystroke costs about 2 µs each way, independent of document size.
- A documented Yjs schema (`Y.Map` blocks, `Y.Text` with marks as formatting and embeds as values, nested `Y.Array`s) that JavaScript Yjs peers read and write directly.
- Out-of-order and duplicate updates converge: updates wait until what they build on arrives, working around yrs 0.27–0.28 integrating them early.
- Blocks that break the schema are hidden, not deleted, and reappear once valid. Concurrent moves of one block keep the copy with the smallest yrs item id.
- Shared undo and redo through a yrs `UndoManager` scoped to local changes, with explicit step grouping.
- Property tests for 2- and 4-peer convergence under reordered and duplicated delivery, a Deno test against JavaScript `yjs`, measurements of concurrent-split behaviour, and `divan` benchmarks.
