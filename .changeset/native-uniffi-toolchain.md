---
canvist_ffi: minor
---

# Add the native build pipeline for iOS, Android and Flutter

- New `canvist_ffi` crate exports a placeholder `EditorHandle` (`new`, `insert_text`, `text`, `version`) through UniFFI. It is backed by a plain `String` until the v2 engine replaces it.
- `platforms/apple`: `CanvistFFI.xcframework` (device + simulator arm64), generated Swift bindings in a `Canvist` Swift package, a SwiftUI sample app, and XCTest/XCUITest suites that run on an iOS Simulator.
- `platforms/android`: cargo-ndk builds for `arm64-v8a` and `x86_64`, generated Kotlin bindings in a `:canvist` Android library, a Compose sample app, and instrumented tests that run on an emulator.
- `platforms/flutter`: a `canvist_flutter` Dart package that binds the handle through flutter_rust_bridge v2 and builds Rust with Dart native-assets build hooks, plus an example app whose `integration_test` runs on both simulators.
- New devenv scripts `build:ios`, `build:android`, `test:ios`, `test:android` and `test:flutter`, a path-gated `native` CI workflow, and a native toolchain guide in the docs.
