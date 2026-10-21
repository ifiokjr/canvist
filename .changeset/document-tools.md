---
"canvist_extensions": feat
"canvist_layout": feat
"canvist_headless": feat
"@canvist/headless": feat
"canvist-dart": feat
---

# Add find and replace, a table of contents, comments, block moves, and clear formatting

Five document tools, each behind its own Cargo feature and included in `full`:

- **`FindAndReplace` (`findAndReplace`).** `setSearchTerm`, `setCaseSensitive`, `nextSearchResult`, `previousSearchResult`, `replace`, `replaceAll`, and the `searchResults` query.
  - The term lives in the editor's settings, outside the document and its history.
  - Matching ignores case unless asked not to.
  - `replaceAll` is one undo step.
- **`TableOfContents` (`tableOfContents`).** The `tableOfContents` query lists headings with their level, text, outline number, and the one the selection is under. `goToHeading` moves the caret to one.
- **`Comments` (`comments`).** A `comment` mark whose `threads` attribute lists thread ids, so comments can overlap.
  - Commands: `setComment` and `unsetComment`. Queries: `activeComments` and `comments`.
  - HTML writes `<span data-type="comment" data-threads="…">`.
  - The StarterKit theme tints commented text.
- **`DragHandle` (`dragHandle`).** `moveBlockUp` and `moveBlockDown` (Alt-↑ and Alt-↓), and `moveBlocksTo` for drops. They move the blocks a text or block selection covers, such as a selected horizontal rule. A block alone in its container moves its container.
- **`ClearFormatting` (`clearFormatting`).** `unsetAllMarks`, `clearNodes` (paragraphs, lifted out of quotes and lists), and `clearFormatting` on `Mod-\`.
