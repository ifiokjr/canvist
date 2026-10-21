---
"canvist_core": minor
---

# Add the `Editor`: history, keymaps, input and paste rules, and the `Extension` trait

- `Editor` and `EditorBuilder` assemble the schema, commands, key bindings, and rules from extensions, sorted by priority, and route every update through one place that runs extension fix-ups and records history.
- `History` is the boundary the collaboration crate replaces; `UndoHistory` groups typing (including IME composition) and deletions by time (500 ms by default), keeps other edits as their own steps, restores the selection, and limits depth.
- `Keymap` parses ProseMirror-style bindings such as `Mod-Shift-z`, with `Mod` meaning Cmd on Apple platforms and Ctrl elsewhere. `BaseKeymap` binds each platform's standard caret, deletion, Enter, select-all, and Escape keys.
- `InputRule` rewrites text as it is typed (block type, wrapping, marks, text replacement, and atomic blocks), runs only on whitespace or punctuation, and skips code blocks. Backspace right after a rule restores the typed text. `PasteRule` formats pasted text, for example into links.
