---
"canvist_extensions": feat
"canvist_layout": feat
"canvist_model": feat
"canvist_headless": feat
"@canvist/headless": feat
"canvist-dart": feat
"canvist_canvas": none
"canvist_canvas_wasm": none
"canvist_paint": none
"canvist_web": none
"@canvist/web": perf
---

# Add tables

`Table` (feature `table`, included in `full`) is TipTap's TableKit. It adds a `table` of `tableRow`s holding `tableCell`s and `tableHeader`s, each cell holding blocks, with TipTap's `colspan`, `rowspan`, and `colwidth` attributes.

- **Commands:**
  - `insertTable`;
  - `addColumnBefore`, `addColumnAfter`, `deleteColumn`;
  - `addRowBefore`, `addRowAfter`, `deleteRow`;
  - `deleteTable`, `mergeCells`, `splitCell`, `mergeOrSplit`;
  - `toggleHeaderRow`, `toggleHeaderColumn`, `toggleHeaderCell`;
  - `setCellAttribute`, `goToNextCell`, `goToPreviousCell`, `fixTables`.
- **Query:** `tableCell` returns where the selection head sits, with its spans and the table's size.
- **Selections:** a selection from one cell to another selects the rectangle of cells between them.
- **Keys:** Tab moves to the next cell and adds a row past the last. Backspace and Delete never join text across a cell's edge, and deleting a selection across cells clears them.
- **Headers:** toggling the header row or column leaves the corner cell to the other one when it is on, as TipTap does, so toggling twice changes nothing.
- **Repairs:** short rows are padded with empty cells after every structural change, including a gap left of a cell that spans down from the row above.

`canvist_model` adds `TableMap`, which places a table's cells on its grid from their spans.

Layout places tables on a grid:

- Columns take their `colwidth`, or an even share of what is left.
- Each row is as tall as its tallest cell, and spanning cells grow their last row.
- Grid lines are drawn as CSS's collapsed borders.
- Hit testing, arrow keys, incremental relayout, and splicing all follow the grid.
- A keystroke in a table of 1,000 cells relays out in about 7 µs.

The HTML renderer writes a fixed-layout `<table>` with the same column widths, and gives header cells a `scope`: `col` across the first row and `row` down the first column. The StarterKit theme draws hairlines and tints header cells.

HTML reads and writes `<table>` as TipTap does, including bare text in cells and a row that a merged cell spans entirely. Markdown writes a GitHub pipe table when the table fits one exactly, and HTML when it does not. It reads both back, with column alignment.

Grid layout sits behind a new default `canvist_layout` feature, `tables`, which `canvist_canvas`, `canvist_canvas_wasm`, and `canvist_paint` turn on. `canvist_web` builds without it, and the `@canvist/web` bundle budget moves to 357,000 gzipped bytes and its wasm budget to 343,000, for the grid hooks every host carries.
