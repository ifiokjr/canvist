All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### A native rich-text editor for Lynx

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #198](https://github.com/canvist/canvist/pull/198)

`@canvist/lynx` puts the iOS and Android `CanvistView`s in Lynx apps as a `<canvist-editor>` element, with the same typed API as `@canvist/web` and `@canvist/react-native`: `CanvistEditor`, `useCanvistEditor`, and `useEditorState` in ReactLynx, and commands, queries, and documents typed from the extensions you register. Calls go through a synchronous `CanvistEditors` native module, which answers with a JSON envelope because Lynx reports native exceptions to the host rather than to JavaScript; failures still arrive as `CanvistError`s with their codes.

On iOS, the `CanvistLynx` pod registers the element and provides `CanvistLynxModule`; on Android, `CanvistLynx.behavior()` and `CanvistLynx.registerModule()` do the same. Both build on the `CanvistHostView` and `HostedEditors` that React Native uses.

`examples/lynx` is the notes screen from the React Native example in ReactLynx, with iOS and Android host apps; its Maestro flows run on a simulator and an emulator in CI. A call from JavaScript into the editor takes 50 to 400 µs at p95 on an iOS Simulator and 260 to 610 µs on an Android emulator (Release builds, averaged over batches, as Lynx's JavaScript engine has no high-resolution clock).
