---
"canvist_core": patch
---

Add text alignment support and accessibility DOM

### Features

- **Text alignment:** Added `set_text_align()` and `text_align()` methods to
  `CanvistEditor`. Supports left, center, right, and justify alignment. The
  layout engine's `x_offset` computation now applies the editor's alignment
  setting. Toolbar alignment buttons (⇤ ⇔ ⇥) are now functional with visual
  feedback showing the active alignment.

- **Accessibility DOM:** Added a hidden `role="document"` container with
  `aria-live="polite"` that mirrors the editor content as `<p>` elements for
  screen readers. This follows the Google Docs pattern of maintaining a parallel
  DOM for assistive technology while rendering to canvas.

- **Screen reader announcements:** Formatting actions (bold, italic, underline,
  strikethrough) now announce state changes via an `aria-live="assertive"`
  status region.
