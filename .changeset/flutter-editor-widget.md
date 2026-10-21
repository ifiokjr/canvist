---
canvist_flutter: feat
---

# Add CanvistEditor, a rich-text editor widget painted from the Rust layout

`canvist_flutter` is now an editor, not just the engine:

- `CanvistEditor` paints canvist's Rust layout with a `DisplayPainter`, measured by `ParagraphMeasurer` with Flutter's own text engine, so lines wrap where Flutter draws them. Only the viewport is laid out, and shaped text runs are cached.
- Input:
  - Text arrives through a delta-model `TextInputClient` that edits the block holding the caret. That covers IME composition, autocorrect, and dictation, and Backspace and Enter from soft keyboards join and split blocks.
  - Hardware keys run the extensions' key bindings.
  - Touch devices get selection handles and the adaptive toolbar.
  - Mouse and trackpad support drag, double click, triple click, shift-click, and a right-click menu.
  - Copy and paste keep formatting within the editor.
- Undo and redo, scrolling that keeps the caret in view, accessibility semantics (text field value, selection, and actions), dark mode, and text scaling. Assistive technology and UI automation that set the field's whole text get the smallest edit, so formatting survives.
- `CanvistController` holds the document and runs typed commands and queries from the engine's handles. It applies Yjs updates and y-sync messages, notifying listeners on every change. `CanvistController.keyPlatform` makes `Mod` Cmd on Apple devices.
- `CanvistStyle` and `CanvistFonts` choose fonts, size, and line height.
- The example app is a full editor with a formatting toolbar and a two-peer collaboration demo.
- Tests:
  - Widget tests cover text input, IME, keys, mouse and touch selection, the toolbar, scrolling, semantics, collaboration, and fast typing.
  - Golden tests compare the editor's layout and pixels with server rendering, both from the painter and from the live widget.
  - Integration tests run on the iOS Simulator and an Android emulator.
  - Patrol tests drive the example app through the device: the platform keyboard, native taps and swipes, long presses, the toolbar, two peers, and system dark mode. They also compare the editor's on-device pixels with server rendering.
  - Keystroke-to-frame budgets run on the host and on devices.
