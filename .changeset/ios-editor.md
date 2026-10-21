---
"canvist_ffi": minor
---

# A native rich-text editor for iOS

The `Canvist` Swift package now ships `CanvistView`, a native UIKit editor, and `CanvistEditor` to show it in SwiftUI.

- **Native text input.** Typing, input methods with marked text, autocorrect, and dictation go through `UITextInput`; the system draws the caret, selection handles, and loupe, and shows the edit menu, with a Format menu for the registered extensions. Hardware keyboard shortcuts run the editor's key bindings, and undo works with the system's gestures.
- **Platform text.** The document is measured with CoreText and painted with Core Graphics, follows Dark Mode and Dynamic Type, scrolls documents of any length, and keeps the caret above the keyboard.
- **VoiceOver** reads the document line by line, with a headings rotor.
- **Task checkboxes** toggle with a tap.
- The example app is a real editor with a toolbar of typed commands, and two editors collaborating over Yjs.
