---
"canvist_headless": minor
"canvist-dart": minor
canvist_flutter: minor
---

# Add `canvist`, a pure Dart package, and rebuild `canvist_flutter` on it

Add `canvist`, a pure Dart package (`platforms/dart`, no Flutter) over `canvist_headless`, and rebuild `canvist_flutter` on top of it.

- The Rust adapter (`canvist_dart`, formerly `canvist_flutter`) binds `canvist_headless` through flutter_rust_bridge and is compiled by the Dart build hook, so `dart run`, `dart test`, and `dart build cli` work on macOS and Linux with the Dart SDK alone. A small loader finds the library through the Dart VM's native-assets mapping, which flutter_rust_bridge's file-name loader cannot.
- `EngineBuilder.add(Extensions.bold)` returns typed handles, generated from the extension metadata, whose methods return typed `Command`s and `Query<T>`s. Engines, documents, formats, UTF-16 positions, snapshots, `CanvistException` with shared `ErrorCode`s, and the Yjs peer and y-sync server API mirror the Rust and TypeScript packages.
- `dart test` passes the shared conformance fixtures (30 of 30), API tests, and an end-to-end test of the `dart:io` example server (Markdown conversion plus a y-websocket endpoint) with two peers over WebSockets. A new `dart` workflow runs them on Linux and macOS without Flutter.
- `canvist_flutter` now depends on and re-exports `canvist`; its example app and integration tests use the real engine instead of the placeholder handle.
- `Engine::named` registers StarterKit extensions by name, shared by the TypeScript and Dart bindings.
