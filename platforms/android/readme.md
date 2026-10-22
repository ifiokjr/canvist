<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/android.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/native.yml"><img src="https://github.com/canvist/canvist/actions/workflows/native.yml/badge.svg?branch=main" alt="native status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist-android

canvist's rich-text editor for Android: `CanvistView`, a native editor for Views and Jetpack Compose over canvist's Rust engine, published as `io.github.ifiokjr:canvist-android` and built from [`canvist_ffi`](../../crates/canvist_ffi).

**Status: available.** See the [Android quick start](../../docs/src/quick-start/android.md).

- **Native text input.** Its own `InputConnection`: Gboard composition, autocorrect and suggestions, voice typing, hardware keyboard shortcuts, the selection handles and magnifier, and the floating text toolbar with formatting.
- **Painted on a `Canvas`, measured with `Paint`,** so lines wrap where Android would wrap them. Dark theme and font scale follow the system.
- **Typed commands.** `StarterKit` and `Extensions` hand out typed handles (`kit.heading.toggleHeading(level = 2)`), generated from the extension metadata.
- **TalkBack** reads and moves through the text by character, word, line, and paragraph, through an `AccessibilityNodeProvider` with a node per block.
- **Collaboration** over Yjs updates or y-sync.
- **Embeddable.** `CanvistView` is a plain `View` with no activity or fragment, so Compose, React Native, and Lynx can host it. `CanvistHostView` and `HostedEditors` are what [`@canvist/react-native`](../../packages/canvist-react-native) builds on: a view started from a JavaScript element's props, and its editor reachable by id from JavaScript's thread. `PaintMeasurer` and `Editor` also work without the view.

```kotlin
val kit = StarterKit()
val view = CanvistView(context, kit.extensions)
view.editor.load(DocFormat.MARKDOWN, "# Hello")
view.editor.run(kit.bold.toggleBold())
```

<details>
<summary><strong>Development</strong></summary>

```sh
./build.sh   # cross-compile the Rust core with cargo-ndk and generate the Kotlin bindings
./test.sh    # the library's and the sample app's instrumented tests on a device or emulator
```

`canvist/src/main/kotlin/io/github/ifiokjr/canvist/Extensions.generated.kt` is generated from the extension metadata; regenerate it with `CANVIST_UPDATE_HANDLES=1 cargo test -p canvist_ffi --test handles`.

</details>
