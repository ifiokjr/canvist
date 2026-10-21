<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/native.yml"><img src="https://github.com/canvist/canvist/actions/workflows/native.yml/badge.svg?branch=main" alt="native status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_ffi

The canvist editor for iOS and Android. It exports an `Editor` through [UniFFI](https://mozilla.github.io/uniffi-rs/) to the `Canvist` Swift package and the `canvist-android` Kotlin library. It isn't published on its own.

An `Editor` is the whole engine without a UI:

- a [`canvist_headless`](../canvist_headless) document with the StarterKit's commands, input rules, history, and key bindings;
- its [`canvist_layout`](../canvist_layout) layout, with text measured by the platform (`CoreTextMeasurer` on iOS, `PaintMeasurer` on Android);
- frames that hand the platform a display list to paint, and only what changed;
- the text model `UITextInput` and `InputConnection` expect: the document as one UTF-16 string, with marked text for IME composition, geometry for carets, selection handles, and candidate windows, and lines and blocks for screen readers;
- Yjs collaboration, through updates or the y-sync protocol.

Commands are typed: `crates/canvist_ffi/tests/handles.rs` generates Swift and Kotlin handles (`kit.heading.toggleHeading(level: 2)`) from the same metadata the Rust commands check their arguments against.

```rust
use std::sync::Arc;

use canvist_ffi::DocFormat;
use canvist_ffi::Editor;
use canvist_ffi::EditorOptions;
use canvist_ffi::testing::FixedPlatformMeasurer;

let editor = Editor::new(EditorOptions::starter_kit(), Arc::new(FixedPlatformMeasurer::default()))?;
editor.load(DocFormat::Markdown, "Hello")?;
editor.select(5, 5)?;
editor.insert_text(" world")?;
assert_eq!(editor.export(DocFormat::Markdown), "Hello world");
# Ok::<(), canvist_ffi::CanvistError>(())
```

Flutter and Dart don't use this crate: they bind `canvist_headless` through `flutter_rust_bridge`. See [the native toolchain guide](../../docs/internal/native-toolchain.md) for building and testing on iOS and Android.
