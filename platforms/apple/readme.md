<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/ios.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/native.yml"><img src="https://github.com/canvist/canvist/actions/workflows/native.yml/badge.svg?branch=main" alt="native status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# Canvist (Swift)

canvist's rich-text editor for iOS: `CanvistView`, a native UIKit editor (with `CanvistEditor` for SwiftUI) over canvist's Rust engine, shipped as an `XCFramework` built from [`canvist_ffi`](../../crates/canvist_ffi).

**Status: available.** See the [iOS quick start](../../docs/src/quick-start/ios.md).

- **Native text input.** `UITextInput` and `UITextInteraction`: the keyboard, input methods (marked text), autocorrect, dictation, selection handles, the loupe, the edit menu with a Format menu, hardware keyboard shortcuts, and undo with the system's gestures.
- **Painted with Core Graphics, measured with CoreText,** so lines wrap where UIKit would wrap them. Dark Mode and Dynamic Type follow the system.
- **Typed commands.** `StarterKit` and `Extensions` hand out typed handles (`kit.heading.toggleHeading(level: 2)`), generated from the extension metadata.
- **VoiceOver** reads the document line by line, with a headings rotor.
- **Collaboration** over Yjs updates or y-sync.
- **Embeddable.** `CanvistView` is a plain `UIScrollView` with no view controller, so SwiftUI, React Native, and Lynx can host it. `CoreTextMeasurer` and `Editor` also work without the view.

```swift
let kit = StarterKit()
let view = try CanvistView(extensions: kit.extensions)
try view.editor.load(format: .markdown, content: "# Hello")
_ = try view.editor.run(command: kit.bold.toggleBold())
```

<details>
<summary><strong>Development</strong></summary>

```sh
./build.sh   # build the Rust core into Frameworks/CanvistFFI.xcframework and generate the Swift bindings
./test.sh    # the package's XCTests (editor and view), then the example app's UI tests on an iOS Simulator
```

`Sources/Canvist/Extensions.generated.swift` is generated from the extension metadata; regenerate it with `CANVIST_UPDATE_HANDLES=1 cargo test -p canvist_ffi --test handles`.

</details>
