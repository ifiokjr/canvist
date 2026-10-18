# Native toolchain (iOS, Android, Flutter)

canvist ships one Rust core to every platform. This page covers the native pipeline: how the core is compiled and bound for iOS (Swift), Android (Kotlin) and Flutter (Dart), and the exact commands to build and test each one on a simulator or emulator.

> **Placeholder API.** The native surface is deliberately tiny while the v2 engine is designed: an `EditorHandle` with `new()`, `insert_text(offset, text)`, `text()` and `version()`, backed by a plain `String`. Offsets count Unicode scalar values. It exists to prove the build pipeline end to end; the real engine will replace it behind the same handle.

## How it fits together

```text
crates/canvist_ffi            Rust handle, exported with UniFFI (proc macros)
 ├─ staticlib ─► platforms/apple     CanvistFFI.xcframework + Swift package `Canvist`
 ├─ cdylib ────► platforms/android   jniLibs/*.so + Android library `:canvist`
 └─ rlib ──────► platforms/flutter   flutter_rust_bridge adapter crate + Dart package `canvist_flutter`
```

| Platform | Binding generator      | Build driver                                                | Package                                    | Tests                                                                      |
| -------- | ---------------------- | ----------------------------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------- |
| iOS      | UniFFI → Swift         | `cargo build` per target + `xcodebuild -create-xcframework` | Swift package `platforms/apple`            | XCTest (package) + XCUITest (SwiftUI sample) on an iOS Simulator           |
| Android  | UniFFI → Kotlin (JNA)  | `cargo ndk` (`arm64-v8a`, `x86_64`)                         | Gradle library `platforms/android/canvist` | androidx.test instrumented tests (library) + Compose UI test (sample)      |
| Flutter  | flutter_rust_bridge v2 | Dart build hook (native assets)                             | Dart package `platforms/flutter`           | `integration_test` (API + widget flow) on an iOS Simulator and an emulator |

Generated sources and native libraries are build outputs and are not committed (`Sources/Canvist/Canvist.swift`, `Frameworks/`, `jniLibs/`, the Kotlin bindings). The one exception is the flutter_rust_bridge output (`platforms/flutter/lib/src/rust`, `platforms/flutter/rust/src/frb_generated.rs`): the adapter crate is a workspace member, so `cargo build --workspace` needs it, and CI checks it is up to date.

## Why flutter_rust_bridge for Flutter

The two candidates were flutter_rust_bridge v2 (FRB) and calling the UniFFI C ABI directly from `dart:ffi`. **We use FRB**, with its native-assets backend:

- **The UniFFI C ABI is not a contract.** Symbol names, checksums, `RustBuffer`/`RustCallStatus` layouts and serialization change between UniFFI releases. Binding it by hand means reimplementing UniFFI's Dart generator and re-validating it on every UniFFI upgrade. The third-party `uniffi-bindgen-dart` that would do this was days old with a few hundred downloads when this was written, too immature for the editor's main mobile surface.
- **FRB generates idiomatic Dart.** Opaque Rust objects become Dart classes with finalizers, errors become typed exceptions, and `#[frb(sync)]` calls are direct synchronous FFI calls, which matters for an editor that will call into layout and hit testing every frame.
- **Native assets replace per-platform glue.** FRB's `native-assets` backend compiles the crate from a Dart build hook (`hook/build.dart`, via `native_toolchain_rust`), so there is no vendored cargokit, podspec or Gradle plugin. `flutter test`, `flutter run` and `flutter build` build the Rust code for whatever device they target.

The cost is a second binding generator. It stays cheap because `platforms/flutter/rust` is a thin adapter: it wraps `canvist_ffi::EditorHandle` and adds no behaviour, so when the engine lands both bindings change in one place. Revisit if UniFFI ever ships an official, stable Dart backend.

## Prerequisites

All platforms:

- The workspace toolchain (`rust-toolchain.toml`, Rust 1.93.1). The build scripts add the cross-compilation targets they need with `rustup target add`.
- `cargo-run-bin` (provided by `devenv`). Pinned tools (`cargo-ndk`, `flutter_rust_bridge_codegen`) install on first use from `[workspace.metadata.bin]`.

iOS (macOS only):

- Xcode with an iOS Simulator runtime: `xcodebuild -downloadPlatform iOS`.
- [XcodeGen](https://github.com/yonaskolb/XcodeGen) for the sample app project (in `devenv` on macOS, or `brew install xcodegen`).

Android:

- Android SDK with `platform-tools`, `emulator`, `cmdline-tools;latest`, `platforms;android-37` and an NDK (r28 or newer, for 16 KB page alignment). Scripts default `ANDROID_HOME` to `~/Library/Android/sdk` and pick the newest NDK under it unless `ANDROID_NDK_HOME` is set.
- JDK 17 or newer. On macOS the scripts fall back to Android Studio's bundled JDK when `JAVA_HOME` is unset.

Flutter:

- [fvm](https://fvm.app). `platforms/flutter/.fvmrc` pins Flutter 3.47.7; run `fvm install` in `platforms/flutter` once. Without fvm the scripts use `flutter` from `PATH`, which must be the same version.

## Build and test

Every command runs from the repository root inside `devenv shell` (or prefix it with `devenv shell --`). Each devenv script wraps a plain shell script under `platforms/`, which you can also run directly.

| Command         | Script                       | What it does                                                                                 |
| --------------- | ---------------------------- | -------------------------------------------------------------------------------------------- |
| `build:ios`     | `platforms/apple/build.sh`   | Builds `aarch64-apple-ios` + `aarch64-apple-ios-sim`, the XCFramework and the Swift bindings |
| `build:android` | `platforms/android/build.sh` | Builds `arm64-v8a` + `x86_64` with cargo-ndk and the Kotlin bindings                         |
| `test:ios`      | `platforms/apple/test.sh`    | `build:ios`, then the package XCTests and the sample app UI test on a simulator              |
| `test:android`  | `platforms/android/test.sh`  | `build:android`, then `./gradlew connectedDebugAndroidTest` on a device or emulator          |
| `test:flutter`  | `platforms/flutter/test.sh`  | `flutter test integration_test` on a simulator and an emulator                               |

### iOS

```sh
test:ios
```

The simulator is the first available iPhone unless `CANVIST_IOS_SIMULATOR` names one (`CANVIST_IOS_SIMULATOR="iPhone 17 Pro"` or a UDID). To work on the sample app in Xcode:

```sh
build:ios
cd platforms/apple/Example && xcodegen generate && open CanvistExample.xcodeproj
```

To use the package from another app, run `build:ios` and add `platforms/apple` as a local Swift package; the `Canvist` product exposes `EditorHandle` and `EditorError`.

### Android

```sh
test:android
```

If no device is connected, the script boots the `canvist-api35` AVD and shuts it down afterwards. When the AVD does not exist it is created from the `system-images;android-35;default;<host abi>` image. To create it by hand:

```sh
sdkmanager --install "system-images;android-35;default;arm64-v8a"   # x86_64 on Intel/Linux hosts
avdmanager create avd --name canvist-api35 --package "system-images;android-35;default;arm64-v8a" --device pixel_6
emulator -avd canvist-api35
```

Set `CANVIST_AVD` to use a different AVD. Recent `avdmanager` releases write AVDs under `~/.config/.android/avd`, which the emulator does not search; the scripts pin `ANDROID_AVD_HOME=~/.android/avd` so both agree. To run the sample app: `build:android`, then `cd platforms/android && ./gradlew :sample:installDebug`.

### Flutter

```sh
test:flutter                                    # iOS Simulator and Android emulator
CANVIST_FLUTTER_PLATFORMS=android test:flutter  # one platform
```

To run the example app: `cd platforms/flutter/example && fvm flutter run`. After changing `platforms/flutter/rust/src/api`, regenerate the bindings and commit the result:

```sh
cd platforms/flutter && cargo bin flutter_rust_bridge_codegen generate
```

`native_toolchain_rust` requires `platforms/flutter/rust/rust-toolchain.toml` with an exact channel and the full target list. Keep its `channel` equal to the workspace `rust-toolchain.toml`.

## Continuous integration

`.github/workflows/native.yml` runs two jobs:

- **ios (simulator)** on `macos-latest`: `platforms/apple/test.sh`, then the Flutter integration tests on iOS.
- **android (emulator)** on `ubuntu-24.04` with KVM: checks the flutter_rust_bridge output is current, builds with cargo-ndk, then runs the Android instrumented tests and the Flutter integration tests on an API 35 `x86_64` emulator.

A full run takes 10-20 minutes, dominated by emulator boot and Gradle. To keep unrelated pull requests fast, the workflow runs on a pull request only when it touches the native surface (`crates/canvist_ffi`, `platforms/`, `Cargo.toml`/`Cargo.lock`, `rust-toolchain.toml` or the workflow), and its jobs are not required checks. The tradeoff is that a core change which breaks the native build is caught after merge rather than before, so the workflow also runs on every push to `main` and nightly (which also catches runner image and toolchain drift). Once `canvist_ffi` depends on the core crates, widen the path filter to `crates/**`, or make the jobs required if they stay fast enough.

## Performance

Every binding runs the same micro-benchmark: per sample, a fresh handle gets 100 single-character `insert_text` appends and then 100 `text()` reads. The per-call time is the sample time divided by 100, and the result is the median (and p90) of 200 samples after 1,000 warm-up calls. `crates/canvist_ffi/benches/round_trip.rs` runs it in Rust with no FFI, so each platform's number minus the Rust number is the binding overhead.

| Run                    | Command                                                                                                                                                                                                                                 |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust baseline          | `cargo bench -p canvist_ffi --bench round_trip`                                                                                                                                                                                         |
| Swift (release)        | `cd platforms/apple && xcodebuild test -scheme Canvist -configuration Release -destination 'platform=iOS Simulator,name=iPhone 17 Pro' -only-testing:CanvistTests/FFIPerformanceTests`                                                  |
| Kotlin (release)       | `cd platforms/android && ./gradlew :canvist:connectedAndroidTest -PcanvistTestBuildType=release -Pandroid.testInstrumentationRunnerArguments.class=io.github.ifiokjr.canvist.FfiPerformanceTest`, then `adb logcat -d -s CanvistPerf:I` |
| Dart (profile, device) | `cd platforms/flutter/example && fvm flutter drive --profile --driver test_driver/integration_test.dart --target integration_test/ffi_performance_test.dart`                                                                            |

The regular test suites also run these benchmarks in debug builds and print `PERF` lines. The assertions only guard against a broken measurement; they never fail on slow hardware.

Baseline measured on 2026-10-18 on an M2 Max, with an iPhone 17 Pro simulator (iOS 26.5) and the `canvist-api35` arm64 emulator:

| Binding                    | Build                      | `insert_text` median (p90) | `text()` median |
| -------------------------- | -------------------------- | -------------------------- | --------------- |
| Rust, no FFI               | release                    | 20 ns (22 ns)              | 27 ns           |
| Swift → UniFFI             | release                    | 120 ns (144 ns)            | 101 ns          |
| Swift → UniFFI             | debug                      | 1.35 µs                    | 0.85 µs         |
| Kotlin → UniFFI (JNA)      | release (non-debuggable)   | 25-28 µs (32-40 µs)        | 22-25 µs        |
| Kotlin → UniFFI (JNA)      | debug                      | 90 µs                      | 82 µs           |
| Dart → flutter_rust_bridge | profile (AOT), emulator    | 0.86 µs (1.5 µs)           | 1.03 µs         |
| Dart → flutter_rust_bridge | debug (JIT), iOS Simulator | 1.56 µs                    | 1.78 µs         |

Library sizes (release, `lto = true`):

| Artifact                                                                         | Size                                                           |
| -------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| `libcanvist_ffi.a`, per XCFramework slice                                        | 16 MiB archive; dead-stripped at app link time                 |
| Linked iOS code (stripped `aarch64-apple-ios` cdylib as a proxy)                 | 346 KiB                                                        |
| `libcanvist_ffi.so` `arm64-v8a` / `x86_64`, stripped (as in the APK)             | 357 KiB / 387 KiB                                              |
| JNA runtime that the Kotlin binding needs                                        | `libjnidispatch.so` 162 KiB (`arm64-v8a`) + 218 KiB of classes |
| `libcanvist_flutter.so` in a release APK, `arm64-v8a` / `x86_64` / `armeabi-v7a` | 565 KiB / 598 KiB / 414 KiB                                    |
| `canvist_flutter.framework` in a release iOS build                               | 549 KiB                                                        |

### What the real binding must avoid

- **JNA struct marshalling on Android.** A JNA call with primitive arguments costs about 50 ns. However, each JNA `Structure` costs about 4 µs. UniFFI's Kotlin code allocates a `RustCallStatus` for every crossing and passes `RustBuffer`s by value, and each method makes three crossings: clone the handle, allocate the string buffer, then the call itself. That is where the 25 µs goes. The fix is to keep UniFFI for coarse API calls and route per-keystroke and per-frame calls through a JNI fast path (for example the `jni` crate with primitive arguments and direct `ByteBuffer`s) or a batched event buffer. Measure on a mid-range device first; emulators on Apple silicon run close to native speed, so real devices are likely slower.
- **Three FFI crossings per UniFFI method call.** On every method call, Swift and Kotlin first call `uniffi_*_fn_clone_*` (an `Arc` increment), and a `String` argument costs another call to allocate the `RustBuffer`. Primitive-only arguments avoid the second crossing.
- **Copies on string arguments.** Swift copies through `utf8CString` and then into a Rust-allocated buffer. Kotlin encodes UTF-16 to UTF-8 and copies into the `RustBuffer`. flutter_rust_bridge serialises into an SSE byte buffer, copies it to native memory and decodes a new `String`. That is two or three allocations and copies per keystroke on every platform.
- **Copies on string returns.** `text()` clones the whole document in Rust, and each binding copies it again. Any UI that re-reads the full text per keystroke pays O(n) two or three times. The engine API should return deltas or dirty ranges, not the full string.
- **The placeholder's linear-time edits.** `insert_text` scans the text to map the offset, then shifts the tail. The engine needs an indexed structure (for example a rope with cached char and UTF-16 offsets).
- **Binary size.** Most of the roughly 350 KiB per ABI is the Rust standard library and UniFFI runtime. `panic = "abort"`, `opt-level = "s"` and `strip = true` on a native-only profile are the obvious next steps. They are not applied yet because the release profile is shared with the wasm build. Apps must also set `abiFilters` to the ABIs we build: JNA's AAR ships `libjnidispatch.so` for seven ABIs, and without a filter the app installs on devices it cannot run on.

## Troubleshooting

- **`No available simulator`**: install a runtime with `xcodebuild -downloadPlatform iOS`, or set `CANVIST_IOS_SIMULATOR`.
- **`Unknown AVD name`**: the AVD was created in a different AVD home. Check `emulator -list-avds` and `ANDROID_AVD_HOME`.
- **`requires libraries and applications that depend on it to compile against version 37`**: install `platforms;android-37` with `sdkmanager`.
- **`The rust-toolchain.toml does not include the target`** (Flutter): add the target to `platforms/flutter/rust/rust-toolchain.toml`.
