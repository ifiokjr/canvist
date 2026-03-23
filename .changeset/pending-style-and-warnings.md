---
"canvist_core": patch
---

Add pending style for collapsed-cursor formatting and clean up WASM warnings

### Pending style

Toggling bold, italic, underline, or strikethrough with a collapsed cursor
(no selection) now sets a "pending style" that is automatically applied to
the next text insertion. This matches the behavior of Google Docs and other
rich-text editors where you can press Ctrl+B, type a word, and have it
appear bold.

- `toggle_bold/italic/underline/strikethrough()` set `pending_style` when
  cursor is collapsed.
- `is_bold/italic/underline()` reflect the pending state for toolbar feedback.
- `insert_text_at()` consumes the pending style and formats the new text.
- Moving the cursor or changing selection clears the pending style.

### Warning cleanup

All compiler warnings in `canvist_wasm` have been resolved:
- Removed unused `offset` variables in bracket auto-close and snippet expand.
- Added `#[allow(dead_code)]` to `LayoutConstants::new/with_zoom`.
- Added module-level `#![allow(dead_code)]` to `dom.rs` (future-use scaffolding).
- Prefixed unused `chars` variable in regex search.
