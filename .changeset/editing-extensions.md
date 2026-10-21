---
"canvist_core": feat
"canvist_extensions": feat
"canvist_layout": feat
"canvist_headless": feat
"@canvist/headless": feat
"canvist-dart": feat
"@canvist/web": perf
---

# Add text alignment, typography, character count, and placeholders

Four editing extensions beyond the StarterKit, each behind its own Cargo feature in `canvist_extensions` and part of `full`:

- `TextAlign`: a `textAlign` attribute on paragraphs and headings, with `setTextAlign`, `toggleTextAlign`, `unsetTextAlign`, a `textAlign` query, and `Mod-Shift-l`, `e`, `r`, and `j`. HTML reads and writes TipTap's `style="text-align: …"`.
- `Typography`: TipTap's 22 smart punctuation rules plus Remirror's `<->`, configurable with `without` and `with_output`. Rules that end in a digit fire on the space or punctuation after them.
- `CharacterCount`: `characters` (grapheme clusters), `words`, and `characterLimit` queries, and `setCharacterLimit`. Edits that would go past the limit don't apply.
- `Placeholder`: hint text for empty blocks, with TipTap's `showOnlyCurrent` and `includeChildren`, a `placeholder` query, and `setPlaceholder`.

`canvist_extensions::extras()` lists every extension beyond the StarterKit with its default configuration.

Supporting changes:

- `canvist_core`:
  - `Extension::update_filters` lets an extension refuse a local update after its fix-ups, like TipTap's `filterTransaction`. The update is undone, the command reports that it did not apply, and `Editor::can` answers the same way. The filters are collected when the editor is built, so an editor whose extensions have none pays nothing per update.
  - `EditorState::settings`, `EditorTransaction::set_setting`, and `commands::set_setting` keep per-editor values such as a limit or placeholder text outside the document, its history, and collaboration. Dry runs restore them.
  - Extension metadata now lists only the nodes the built schema has.
  - `InputRule::on` names the typed characters that can complete a rule whose pattern ends in a group or class, so it doesn't run on every punctuation keystroke.
  - Input rules no longer panic when their 64-byte look-back starts inside a multi-byte character, which `**bold**` after a line of `€` or CJK text could trigger.
- `canvist_layout`:
  - Lines align to a block's `textAlign` attribute: left, centre, right, or justified by widening spaces. Server-rendered HTML writes the matching `text-align`.
  - A row now rereads its `dir` and `textAlign` attributes when the block changes. Before, changing `dir` kept the old direction until the row was rebuilt.
  - `DisplayOptions::placeholder` draws placeholder text, faded, in an empty block.
- Clipboard: pasted block attributes keep only the ones the block type declares.

The `@canvist/web` bundle budget moves to 354,000 gzipped bytes, for update vetoes, editor settings, `can()` dry runs, and text alignment, which every host runs.
