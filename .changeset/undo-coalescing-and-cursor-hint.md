---
"canvist_core": minor
---

Add undo coalescing and improved cursor positioning after undo/redo

### Undo coalescing

Consecutive single-character inserts that arrive within a configurable timeout
window (default 500ms) are now merged into a single undo group. This gives
users a natural "word at a time" undo experience instead of undoing one
character at a time.

- Newlines, multi-character inserts, deletions, and formatting operations all
  break the coalescing chain and start a new undo group.
- The timeout is configurable via `set_coalesce_timeout_ms()`.
- `Transaction::compose()` enables merging inverse transactions.

### Cursor positioning

Undo and redo now restore the cursor to a meaningful position instead of
jumping to the end of the document:
- After undoing an insert, the cursor is placed at the insert position.
- After undoing a delete, the cursor is placed after the restored text.
- `Transaction::cursor_hint()` computes the appropriate position from
  the transaction's operations.
