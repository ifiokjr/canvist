# Changelog

All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Add CanvistEditor, a rich-text editor widget painted from the Rust layout

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #167](https://github.com/canvist/canvist/pull/167)

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

#### Add `canvist`, a pure Dart package, and rebuild `canvist_flutter` on it

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #113](https://github.com/canvist/canvist/pull/113)

Add `canvist`, a pure Dart package (`platforms/dart`, no Flutter) over `canvist_headless`, and rebuild `canvist_flutter` on top of it.

- The Rust adapter (`canvist_dart`, formerly `canvist_flutter`) binds `canvist_headless` through flutter_rust_bridge and is compiled by the Dart build hook, so `dart run`, `dart test`, and `dart build cli` work on macOS and Linux with the Dart SDK alone. A small loader finds the library through the Dart VM's native-assets mapping, which flutter_rust_bridge's file-name loader cannot.
- `EngineBuilder.add(Extensions.bold)` returns typed handles, generated from the extension metadata, whose methods return typed `Command`s and `Query<T>`s. Engines, documents, formats, UTF-16 positions, snapshots, `CanvistException` with shared `ErrorCode`s, and the Yjs peer and y-sync server API mirror the Rust and TypeScript packages.
- `dart test` passes the shared conformance fixtures (30 of 30), API tests, and an end-to-end test of the `dart:io` example server (Markdown conversion plus a y-websocket endpoint) with two peers over WebSockets. A new `dart` workflow runs them on Linux and macOS without Flutter.
- `canvist_flutter` now depends on and re-exports `canvist`; its example app and integration tests use the real engine instead of the placeholder handle.
- `Engine::named` registers StarterKit extensions by name, shared by the TypeScript and Dart bindings.

#### Paint ink strokes on iOS, Android, and in Flutter

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #179](https://github.com/canvist/canvist/pull/179)

Display lists now carry ink strokes past the web canvas to the native and Flutter hosts.

- `canvist_ffi` adds `DisplayItem::Stroke` with the stroke's points, width, colour, and bounds. The iOS and Android `DisplayPainter`s draw it as a polyline with round caps and joins.
- `canvist-dart` adds `ViewItemKind.stroke`, with `ViewItem.points` as `x, y` pairs and `ViewItem.lineWidth`. The item's rectangle bounds the points and caps.
- `canvist_flutter`'s `DisplayPainter` draws strokes the same way. A stroke of one point is a dot.

### 🐛 Fixed

- **Stop restarting Android keyboards while the editor has focus.** Android keyboards mark the word at the caret when their input starts, for suggestions. `CanvistEditor` sent its own state back without that mark, which made Android restart the keyboard, which marked the word again, several times a second for as long as the editor had focus. The keyboard and its suggestion strip churned, and suggestions and autocorrect kept resetting. The editor now leaves the word the keyboard marks alone, and composing over it works as before. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #187](https://github.com/canvist/canvist/pull/187)

#### Publish the Dart and Flutter packages to pub.dev

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #150](https://github.com/canvist/canvist/pull/150)

The `canvist` package's Rust crate no longer inherits anything from the Cargo workspace. It names `canvist_headless` by the exact crates.io release it was generated against, which monochange bumps on every release, and the workspace's `[patch.crates-io]` builds the local sources inside this repository. So the build hook compiles the crate from the pub cache, where the workspace does not exist.

Both packages now ship a `LICENSE`, a `README.md`, and a repository link, and allow `flutter_rust_bridge` patch releases. CI validates both archives with `pub publish --dry-run` and builds and tests the `canvist` archive outside the repository. The release workflow publishes them after crates.io.

- **Keep a selection the app sets while the keyboard is still starting.** A keyboard's first updates after the editor gains focus describe the caret the editor opened with. When an app set a new selection in that moment, for example selecting everything as the editor opens, those late updates arrived after it and collapsed the selection, so the next format applied to nothing. `CanvistEditor` now recognises an update that only returns to a selection it has since replaced, and keeps its own. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #195](https://github.com/canvist/canvist/pull/195)

## 0.1.0

- First version: the Flutter binding for canvist, built on the pure Dart `canvist` package.
