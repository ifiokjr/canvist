---
"canvist_ffi": minor
"canvist_headless": minor
---

# The full editor engine for iOS and Android

`canvist_ffi`, the native binding behind the `Canvist` Swift package and the `canvist-android` Kotlin library, now runs the real editor instead of a placeholder string. An `Editor` owns a document with the StarterKit's commands, lays it out with `canvist_layout`, and hands the platform a display list to paint.

- **Typed commands in Swift and Kotlin.** Generated handles (`kit.bold.toggleBold()`, `kit.heading.toggleHeading(level: 2)`) build commands checked against the same metadata as Rust, TypeScript, and Dart. Queries decode to typed values.
- **Platform text.** `CoreTextMeasurer` and `PaintMeasurer` measure with each platform's own text engine, so lines wrap where UIKit and Android would wrap them.
- **Platform text input.** The document is exposed the way `UITextInput` and `InputConnection` address text, with marked text for IME composition, autocorrect replacements, and the geometry for carets, selection handles, and candidate windows.
- **Accessibility.** Blocks with their roles (headings, list items, quotes, code) and visual lines, for VoiceOver and TalkBack.
- **Collaboration.** Editors are Yjs peers, through updates or the y-sync protocol.

`canvist_headless` gains `Doc::can_undo` and `Doc::can_redo`, which answer from the shared history on a collaborative document.
