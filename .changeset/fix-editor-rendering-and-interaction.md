---
"canvist_core": minor
---

Fix selection-scoped formatting, mouse interaction, and toolbar layout

### Breaking changes to `Document::apply_style`

`apply_style()` now properly splits text runs at selection boundaries when applying
formatting to a partial run. Previously it merged the style into the entire overlapping
run, causing formatting to "leak" beyond the selected text.

### Bug fixes

- **Formatting precision:** Selecting a portion of text and toggling bold/italic/underline
  now only affects the selected characters, not the entire text run. Runs are split into
  up to 3 pieces (before, selected, after) as needed.
- **Undo/redo:** `restore_run_styles()` now handles split runs by merging them back into
  the original run shape when undoing a format operation.
- **Double-click selection:** Double-clicking to select a word no longer loses the selection
  when the click event fires after mousedown.
- **Triple-click-and-drag:** Triple-clicking now enters line-level selection mode. Dragging
  after a triple-click extends the selection line-by-line (standard macOS behavior).
  Double-click-and-drag extends word-by-word.
- **Toolbar overflow:** The formatting toolbar now wraps to multiple rows with grouped
  controls instead of overflowing the viewport.
- **IME/composition:** Full `compositionstart`, `compositionupdate`, and `compositionend`
  event handling for CJK and other IME input methods.
