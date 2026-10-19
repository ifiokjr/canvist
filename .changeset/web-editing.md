---
"@canvist/web": minor
"canvist_web": minor
"canvist_headless": minor
---

# Edit on the web: EditContext input, IME, the clipboard, drag and drop, and a screen-reader mirror

- Text arrives through `EditContext` in Chromium, with the paragraph and caret bounds the platform's input methods need, and through a hidden textarea everywhere else. `input: "textarea"` forces the textarea, as a kill switch for `EditContext` problems.
- Compositions from input methods and dead keys show as preedit text and commit in place. Key bindings follow the platform (`Mod` is Cmd on Apple devices), detected from the browser or set with `platform`.
- Click, Shift-click, double-click, and triple-click select; dragging extends by the unit the press selected. Copy, cut, and paste carry HTML and plain text; the selection can be dragged to move or copy it, and drops from other applications land where they fall.
- A visually hidden copy of the document, the same semantic HTML the server renders, is refreshed on idle for screen readers and find-in-page.
- Every input event is painted in its own task. `editor.inputLatency()` reports input-to-paint times; the browser tests hold their 95th percentile under 8 ms and type a 1,000-key burst without dropping a key.
- `Engine::with_platform` in `canvist_headless` chooses what `Mod` means in key bindings.
