---
canvist-dart: feat
---

# Report an engine's key platform and keep shared undo in the shared history

- `Engine.platform` says which platform's key bindings an engine follows.
- An `EditorView` on a shared document sends `Mod-z`, `Mod-Shift-z`, `Mod-y`, and the `undo` and `redo` commands through the shared history, so undo never reverts a collaborator's changes.
- `EditorView.caretRect` and `selectionEnds` lay out the blocks they need, so a host can scroll to a caret far outside the viewport.
