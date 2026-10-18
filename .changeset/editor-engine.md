---
"canvist_editor": minor
"canvist_model": minor
---

Add `canvist_editor`, the platform-agnostic editing engine, and `Transaction::allocate_id` in `canvist_model` so commands can create blocks inside a transaction.

- `EditorState` holds the document, selection, stored marks, and IME composition. Every change goes through `EditorState::change` or `EditorState::apply_steps`, which map the selection and composition, clear stale stored marks, and keep the document non-empty.
- Commands as plain functions: typing with mark inclusivity, deleting by grapheme, word, line, or document (joining blocks and lifting out of containers at block boundaries), marks, select all, block selections, and IME composition.
- Selection motion by grapheme cluster and word (UAX #29), line, and document, treating embeds and atomic blocks as single stops. Vertical motion goes through the `LineNavigator` trait, with a paragraph-based fallback for headless use.
- An outline notation for before/after command tests.
