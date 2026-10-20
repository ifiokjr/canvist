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

The native binding surface for canvist's iOS and Android packages. It exports an `EditorHandle` through [UniFFI](https://mozilla.github.io/uniffi-rs/), which the `Canvist` Swift package and the Kotlin library wrap. It isn't published on its own.

**The handle is a placeholder for now.** It holds plain text, which proves the native build and test pipelines end to end. The v2 editor engine replaces it behind the same handle. Flutter and Dart don't use this crate: they bind [`canvist_headless`](../canvist_headless) through `flutter_rust_bridge`.

See [the native toolchain guide](../../docs/internal/native-toolchain.md) for building and testing on iOS and Android.
