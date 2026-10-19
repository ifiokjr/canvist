---
"canvist_collab": minor
"canvist_scene": minor
---

# Collaborate on canvas scenes with `SceneBinding`

A new `scene` feature adds `SceneBinding`, which mirrors `canvist_scene` steps to a Yjs document and back with the same guarantees as the document binding: exact collaborative undo of your own changes, reordering of updates that arrive early, and convergence however updates interleave.

- Each node is a `Y.Map` in one flat `"nodes"` map. Its position is an atomic `[parent, key, clock, client, previousParent]` register, and each property is its own last-writer-wins entry.
- Text frames keep their text inside the scene's Yjs document, in exactly the document binding's shape, so creating a frame with its first paragraph is one atomic change.
- Orphans, unknown types, and nodes their parent rejects are hidden but kept. When concurrent moves form a cycle, the later move loses.
- The block binding's mirror is split from its Yjs document so both bindings share it. `Binding`'s API is unchanged.

`canvist_scene` gains `Transaction::text_external`, which applies a collaborator's text change to a text frame through the frame's own document, and `Prop::fits`, which checks a property value against a node kind.
