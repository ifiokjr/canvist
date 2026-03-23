---
"canvist_core": patch
---

Rich-text clipboard paste and integration tests

### Rich-text paste

The demo's Ctrl+V handler now attempts to read HTML from the clipboard via the
Clipboard API (`clipboard.read()` with `text/html` MIME type) before falling
back to plain text. When HTML is available, it's passed to `paste_html()` which
preserves bold, italic, underline, and strikethrough formatting from the source.

### Integration tests

Added 3 new runtime integration tests:
- `format_partial_then_undo_preserves_text` — format a substring, undo, verify
  all runs are unstyled and text is preserved
- `insert_newline_creates_paragraph_then_undo_merges` — verify paragraph
  splitting and merging through undo
- `multiple_operations_undo_redo_roundtrip` — insert → format → insert → undo
  chain with redo verification
