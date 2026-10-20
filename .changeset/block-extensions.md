---
"canvist_extensions": feat
"canvist_layout": feat
"canvist_headless": feat
"@canvist/headless": feat
"canvist-dart": feat
---

# Add details, callouts, and math

Three block extensions, each behind its own Cargo feature and included in `full`:

- `Details` (`details`): TipTap's `details` block, holding a `detailsSummary` and a `detailsContent`, with an `open` attribute. Use `setDetails`, `unsetDetails`, `toggleDetailsOpen`, and the `detailsOpen` query. Enter in a summary opens the details and moves into the content. Layout draws a closed one as its summary beside a `▸` disclosure marker and lays out none of the hidden content.
- `Callout` (`callout`): Remirror's tinted box, with a `type` of `info`, `warning`, `error`, `success`, or `blank`. Use `toggleCallout`, `setCalloutType`, and the `calloutType` query. Typing `:::warning` at the start of a line makes one, and the StarterKit theme colours each type.
- `Mathematics` (`mathematics`): TipTap's `inlineMath` embed and `blockMath` block, holding LaTeX in `latex`. Use `insertInlineMath` and `insertBlockMath`, or type `$$x^2$$` and `$$$x^2$$$`. Layout draws inline LaTeX as its source and reserves a slot for block formulas, so the host can typeset them.

HTML reads and writes TipTap's markup:

- `<details>` with a `<summary>` and `<div data-type="detailsContent">`;
- `<div data-callout-type>`;
- `<span data-type="inline-math">` and `<div data-type="block-math">`, with the source in `data-latex`.

Markdown writes callouts as GitHub alerts (`> [!WARNING]`), details as GitHub's `<details>` HTML, and formulas as `$x$` and `$$` blocks, and reads all three back. A price such as `$5` stays text.
