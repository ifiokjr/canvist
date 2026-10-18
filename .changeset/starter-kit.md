---
"canvist_extensions": minor
"canvist_editor": minor
---

Add `canvist_extensions`, the StarterKit, with each extension behind its own Cargo feature.

- Nodes: `Document`, `Paragraph`, `Heading`, `Blockquote`, `CodeBlock`, `HardBreak`, `HorizontalRule`, `BulletList`, `OrderedList`, `ListItem`, and `TaskList`, with TipTap's names, key bindings, and Markdown-style input rules.
- Marks: `Bold`, `Italic`, `Underline`, `Strike`, `Code`, and a non-inclusive `Link` with autolinking and linking on paste.
- Functionality: `History` keys and `TrailingNode`.
- `canvist_editor` gains `UndoInputRule` (Backspace takes back an input rule before any other Backspace binding), `lift_empty_block`, and a public `wrap_in_list`, and its hot paths avoid scanning sibling lists so a keystroke costs the same at 100 or 100,000 blocks.
- A scripted-typing harness (`"## Hi⏎- one⏎two⇥"`) with JSON snapshots, property tests that random keystroke scripts keep every invariant and undo and redo exactly, and `divan` keystroke benchmarks at 100, 10,000, and 100,000 blocks.
