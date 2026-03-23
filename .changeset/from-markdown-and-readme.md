---
"canvist_core": minor
---

Add Markdown import/paste and improved README

### Markdown import

- `Document::from_markdown()` parses `**bold**`, `*italic*`, `~~strike~~`,
  and double-newline paragraph breaks into the document model.
- `parse_simple_markdown()` exposed as a public utility.
- `CanvistEditor::paste_markdown()` WASM method for pasting Markdown with
  formatting preserved.

### README

Rewrote the `@canvist/canvist` package README with:
- Feature highlights (rendering, formatting, undo, collab, a11y, paste)
- Quick start example with formatting
- Collaboration code example
- API reference table
- Links to both demos (editor + collab)
