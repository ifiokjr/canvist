---
"canvist_core": minor
---

Proper paragraph splitting and merging for multi-paragraph documents

### Paragraph splitting on newline

`Document::insert_text()` now creates separate `Paragraph` nodes when the
inserted text contains newlines, instead of embedding `\n` characters into a
single text run. This gives the document tree proper semantic structure:

```text
Before: Root → Para → Run("hello\nworld")
After:  Root → Para → Run("hello")
             → Para → Run("world")
```

### Paragraph merging on delete

`Document::delete()` now detects when a deletion crosses a paragraph boundary
(spans a `\n` in the plain text) and merges the affected paragraphs by moving
the second paragraph's runs into the first. Empty paragraphs are cleaned up
automatically via `merge_empty_paragraphs()`.

### Updated serialization

`to_html()` and `to_markdown()` now iterate the paragraph tree directly
instead of splitting on `\n` within run text, producing correct multi-paragraph
output.

### Updated `paragraph_count()`

Now returns the actual number of `Paragraph` nodes in the tree (with a minimum
of 1 for empty documents).
