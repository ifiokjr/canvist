# canvist_extensions

The [canvist](https://github.com/ifiokjr/canvist) StarterKit: the extensions TipTap's StarterKit ships, built on [`canvist_editor`](../canvist_editor) and named the way TipTap names them, so documents move between the two as ProseMirror JSON.

- **Nodes:** paragraph, heading (levels 1 to 6), blockquote, code block, hard break, horizontal rule, bullet list, ordered list, list item, and task list.
- **Marks:** bold, italic, underline, strike, code, and link (non-inclusive, with autolinking and linking on paste).
- **Behaviour:** Markdown-style input rules (`##`, `-`, `1.`, `[ ]`, `>`, `` ``` ``, `---`, `**bold**`), TipTap's key bindings, undo and redo keys, and a trailing paragraph.

Each extension is behind its own Cargo feature, and `starter-kit` (the default) enables them all, so an app compiles only what it registers.

```rust
use canvist_editor::Editor;
use canvist_extensions::StarterKit;

let editor = Editor::builder()
	.extensions(StarterKit::default().extensions())
	.build()?;
```

Typing a keystroke through the whole kit (keymaps, input rules, history, and fix-ups) costs about 0.6 µs at 100 to 100,000 blocks; a space, which runs every input rule, about 3.5 µs. Run `cargo bench -p canvist_extensions` to reproduce.
