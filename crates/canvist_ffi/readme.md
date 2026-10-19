<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img alt="canvist" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" width="220">
    </picture>
  </a>
</p>

# canvist_ffi

> Native binding surface for the canvist canvas editor.

`canvist_ffi` exports an `EditorHandle` through [UniFFI](https://mozilla.github.io/uniffi-rs/) for Swift (iOS) and Kotlin (Android). The Flutter package wraps the same handle through `flutter_rust_bridge`.

**This is a placeholder.** The handle is backed by a plain `String` so the native build pipelines can be proven before the v2 editor engine exists. The engine will replace it behind the same handle.

See [the native toolchain guide](../../docs/src/platforms/native-toolchain.md) for building and testing on iOS, Android and Flutter.
