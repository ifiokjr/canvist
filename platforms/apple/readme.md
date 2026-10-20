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

The Swift package for canvist on iOS. It wraps the Rust core through [`canvist_ffi`](../../crates/canvist_ffi), shipped as an `XCFramework`.

**Status: coming soon.** Today the package proves the build, binding, and simulator test pipeline end to end, with a placeholder handle that holds plain text. The native editor arrives behind the same `Canvist` API. See the [iOS quick start](../../docs/src/quick-start/ios.md).

<details>
<summary><strong>Development</strong></summary>

```sh
./build.sh   # build the Rust core into Frameworks/CanvistFFI.xcframework and generate the Swift bindings
./test.sh    # the package's XCTests, then the example app's UI test on an iOS Simulator
```

</details>
