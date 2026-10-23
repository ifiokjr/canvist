---
sdk: feat
---

# Release the SDK to every registry from one release pull request

Releases move from knope to monochange. One version covers every SDK package, and the release pull request publishes each one:

- The Rust crates `canvist_model`, `canvist_core`, `canvist_extensions`, `canvist_layout`, `canvist_collab`, and `canvist_headless` go to crates.io, and `@canvist/headless` goes to npm and JSR. All three registries use trusted publishing.
- The Android library goes to Maven Central as `io.github.ifiokjr:canvist-android`.
- The Swift package's `CanvistFFI.xcframework.zip`, its checksum, and `Canvist.swift` are attached to the GitHub release.
- The Dart packages are ready to reserve their pub.dev names. `canvist_flutter` now depends on `canvist: ^0.1.0` and resolves the local package through `pubspec_overrides.yaml`.
