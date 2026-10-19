---
"canvist_extensions": minor
"canvist_editor": patch
---

# Add clipboard formats to `canvist_extensions` behind the `clipboard` feature

Add clipboard formats to `canvist_extensions` behind the `clipboard` feature (part of `starter-kit`).

- `clipboard::to_html`, `to_markdown`, and `to_plain_text` serialize a copied `Slice` in the markup TipTap renders and in CommonMark with GitHub task lists and strikethrough.
- `clipboard::from_html`, `from_markdown`, and `from_plain_text` parse pasted content back into a schema-valid slice: unknown markup degrades to text, bare paragraphs inside lists gain list items, and Google Docs' inline `font-weight`, `font-style`, and `text-decoration` styles become marks.
- The parsers are small and hand-written, adding no dependencies and no HTML5 or CommonMark engine to WebAssembly builds.
- `canvist_editor`'s paste keeps the type of an open first block pasted into an empty block and keeps an explicit empty last block.
