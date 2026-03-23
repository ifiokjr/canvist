---
"canvist_core": patch
---

Add edge-case tests for paragraphs, serialization, and cross-boundary formatting

6 new tests covering critical edge cases:

- **Styled paragraph split:** Inserting a newline inside a bold run preserves
  bold on both resulting paragraphs.
- **JSON roundtrip:** Multi-paragraph document with formatting survives
  `to_json()` → `from_json()` with paragraph count and styles preserved.
- **HTML roundtrip:** Multi-paragraph styled document survives `to_html()` →
  `from_html()` with paragraph breaks and inline formatting preserved.
- **Collaboration sync:** Multi-paragraph document roundtrips through
  `sync_from_document()` → `sync_to_document()` preserving all paragraphs.
- **Delete entire paragraph:** Deleting a paragraph's content plus its trailing
  newline correctly merges remaining paragraphs.
- **Format across boundary:** Applying bold across a paragraph boundary
  (`\n`) correctly styles runs in both paragraphs.
