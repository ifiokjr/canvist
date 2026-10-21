---
"@canvist/web": feat
"@canvist/react": feat
"@canvist/vue": feat
canvist_web: feat
---

# Read-only editors, placeholders, and menus that follow the selection

`@canvist/web` gains the editor features TipTap users reach for first.

- **`editable`**: `createEditor(element, { editable: false })` and `editor.setEditable(editable)` make an editor read-only. People can still select with the pointer and navigation keys and copy, but typing, pasting, cutting, and dropping change nothing, no caret shows, and the field has `aria-readonly`. Commands and the API still change the document, as in TipTap. `editor.editable` reads it.
- **`placeholder`**: dimmed text where the first line starts while the document is empty, set in the body font and line height so it sits where typing begins, and the field's `aria-placeholder`. `editor.setPlaceholder(text)` changes it, and `editor.isEmpty` reports an empty document.
- **`@canvist/web/menus`**: `bubbleMenu(editor, element, options)` keeps an element over selected text, and `floatingMenu` beside the caret on an empty line. They follow edits, scrolling, and resizing, flip to the other side when there is no room, keep the editor's focus when pressed, and take `placement`, `offset`, `padding`, and `shouldShow`. `placeMenu(element, rect, options)` is the positioning on its own.
- **Selection geometry**: `editor.selectionRect()` is the selection's bounding box in document coordinates, `editor.toClientRect(rect)` converts to viewport coordinates, and `editor.blockText()` is the text of the block holding the selection. `caretRect()` and `selectionRect()` now lay out first when an edit is waiting for the next frame, so they describe the edited document.
- **Focus**: `editor.focused` reads focus, and `subscribe` listeners also hear about focus and `editable` changes.
- `@canvist/react` and `@canvist/vue`: `CanvistEditor`'s `editable` and `placeholder` props follow their values, and `BubbleMenu` and `FloatingMenu` components wrap the menus.
