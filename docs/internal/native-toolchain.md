# Native toolchain (iOS, Android, Flutter)

canvist ships one Rust core to every platform. This page covers the native pipeline: how the core is compiled and bound for iOS (Swift), Android (Kotlin) and Flutter (Dart), and the exact commands to build and test each one on a simulator or emulator.

Swift and Kotlin bind the whole engine through `canvist_ffi`: an `Editor` that owns a `canvist_headless` document with the StarterKit's commands, its `canvist_layout` layout, and the Yjs binding. Text is measured by the platform (`CoreTextMeasurer`, `PaintMeasurer`) through the `PlatformMeasurer` callback, and each frame hands the platform a display list to paint. Text input methods address the document as one UTF-16 string (text blocks joined by line feeds), which is what `UITextInput` and `InputConnection` expect. Dart binds `canvist_headless` directly (architecture Decision 9).

## How it fits together

```text
crates/canvist_ffi            the editor (headless + layout + collab), exported with UniFFI
 ├─ staticlib ─► platforms/apple     CanvistFFI.xcframework + Swift package `Canvist`
 └─ cdylib ────► platforms/android   jniLibs/*.so + Android library `:canvist`

crates/canvist_headless       the engine with no UI
 └─ rlib ──────► platforms/dart      flutter_rust_bridge adapter crate + pure Dart package `canvist`
                  └─► platforms/flutter   Flutter package `canvist_flutter`, which re-exports `canvist`
```

| Platform | Binding generator      | Build driver                                                | Package                                    | Tests                                                                      |
| -------- | ---------------------- | ----------------------------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------- |
| iOS      | UniFFI → Swift         | `cargo build` per target + `xcodebuild -create-xcframework` | Swift package `platforms/apple`            | XCTest (package) + XCUITest (SwiftUI sample) on an iOS Simulator           |
| Android  | UniFFI → Kotlin (JNA)  | `cargo ndk` (`arm64-v8a`, `x86_64`)                         | Gradle library `platforms/android/canvist` | androidx.test instrumented tests (library) + Compose UI test (sample)      |
| Dart     | flutter_rust_bridge v2 | Dart build hook (native assets)                             | Dart package `platforms/dart`              | `dart test` (conformance, API, server) on Linux and macOS, no Flutter      |
| Flutter  | (from `canvist`)       | (from `canvist`)                                            | Flutter package `platforms/flutter`        | `integration_test` (API + widget flow) on an iOS Simulator and an emulator |

Generated sources and native libraries are build outputs and are not committed (`Sources/Canvist/Canvist.swift`, `Frameworks/`, `jniLibs/`, the UniFFI Kotlin bindings `canvist_ffi.kt`). The typed extension handles are committed: `crates/canvist_ffi/tests/handles.rs` generates `Extensions.generated.swift` and `Extensions.generated.kt` from the extension metadata, fails when they are out of date, and rewrites them with `CANVIST_UPDATE_HANDLES=1 cargo test -p canvist_ffi --test handles`. The one exception is the flutter_rust_bridge output (`platforms/dart/lib/src/rust`, `platforms/dart/rust/src/frb_generated.rs`): the adapter crate is a workspace member, so `cargo build --workspace` needs it, and CI checks it is up to date.

## Why flutter_rust_bridge for Flutter

The two candidates were flutter_rust_bridge v2 (FRB) and calling the UniFFI C ABI directly from `dart:ffi`. **We use FRB**, with its native-assets backend:

- **The UniFFI C ABI is not a contract.** Symbol names, checksums, `RustBuffer`/`RustCallStatus` layouts and serialization change between UniFFI releases. Binding it by hand means reimplementing UniFFI's Dart generator and re-validating it on every UniFFI upgrade. The third-party `uniffi-bindgen-dart` that would do this was days old with a few hundred downloads when this was written, too immature for the editor's main mobile surface.
- **FRB generates idiomatic Dart.** Opaque Rust objects become Dart classes with finalizers, errors become typed exceptions, and `#[frb(sync)]` calls are direct synchronous FFI calls, which matters for an editor that will call into layout and hit testing every frame.
- **Native assets replace per-platform glue.** FRB's `native-assets` backend compiles the crate from a Dart build hook (`hook/build.dart`, via `native_toolchain_rust`), so there is no vendored cargokit, podspec or Gradle plugin. `flutter test`, `flutter run` and `flutter build` build the Rust code for whatever device they target.

The cost is a second binding generator. It stays cheap because `platforms/dart/rust` is a thin adapter over `canvist_headless` that adds no behaviour. Revisit if UniFFI ever ships an official, stable Dart backend.

The Rust library lives in the pure Dart package, so `dart run`, `dart test`, and `dart build cli` work without Flutter. flutter_rust_bridge's loader opens the library by file name, which finds it inside a Flutter app bundle but not where the Dart VM keeps native assets, so `platforms/dart/lib/src/load.dart` resolves the library through the VM's native-assets mapping on desktop and keeps flutter_rust_bridge's loader on iOS and Android.

## Prerequisites

All platforms:

- The workspace toolchain (`rust-toolchain.toml`, Rust 1.96.0). The build scripts add the cross-compilation targets they need with `rustup target add`.
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

To use the package from another app, run `build:ios` and add `platforms/apple` as a local Swift package; the `Canvist` product exposes `Editor`, the typed handles (`StarterKit`, `Extensions`), and `CoreTextMeasurer`.

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

To run the example app: `cd platforms/flutter/example && fvm flutter run`. After changing `platforms/dart/rust/src/api`, regenerate the bindings and commit the result:

```sh
cd platforms/dart && cargo bin flutter_rust_bridge_codegen generate
```

`native_toolchain_rust` requires `platforms/dart/rust/rust-toolchain.toml` with an exact channel and the full target list. Keep its `channel` equal to the workspace `rust-toolchain.toml`.

## Continuous integration

`.github/workflows/native.yml` runs two jobs:

- **ios (simulator)** on `macos-latest`: `platforms/apple/test.sh`, then the Flutter integration tests on iOS.
- **android (emulator)** on `ubuntu-24.04` with KVM: checks the flutter_rust_bridge output is current, builds with cargo-ndk, then runs the Android instrumented tests and the Flutter integration tests on an API 35 `x86_64` emulator.

A full run takes 10-20 minutes, dominated by emulator boot and Gradle. `canvist_ffi` depends on the core crates, so the workflow runs on a pull request when it touches any crate (`crates/**`), `platforms/`, `Cargo.toml`/`Cargo.lock`, `rust-toolchain.toml` or the workflow. Its jobs are not required checks; the workflow also runs on every push to `main` and nightly, which catches runner image and toolchain drift.

## Performance

Keystroke-to-paint is measured in three places:

| Run                                      | Command                                                                                                          | Budget                   |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------ |
| Rust: keystroke → frame (no FFI)         | `cargo bench -p canvist_ffi`, budgets in `cargo test --release -p canvist_ffi --test perf_budgets -- --ignored`  | 200 µs at 10k paragraphs |
| Swift: keystroke → frame through UniFFI  | `cd platforms/apple && xcodebuild test -scheme Canvist -destination 'platform=iOS Simulator,name=iPhone 17 Pro'` | p95 4 ms on a simulator  |
| Kotlin: keystroke → frame through UniFFI | `cd platforms/android && ./gradlew :canvist:connectedAndroidTest`, then `adb logcat -d -s CanvistPerf:I`         | p95 8 ms on an emulator  |

The Rust benchmarks measure everything between the platform delivering a key and painting: the edit, relayout, the display list, and its damage, for typing, IME composition, the queries `UITextInput` makes after each keystroke, scrolling, and a collaborator's keystroke. The Swift and Kotlin tests add the binding and real text measurement. `-PcanvistTestBuildType=release` runs the Kotlin numbers against a non-debuggable build.

`crates/canvist_ffi/tests/stress.rs` types thousands of random keyboard, IME, autocorrect, undo, and collaboration inputs and checks that nothing fails and the text model always matches the document.

The Dart binding keeps its own benchmark: `cd platforms/flutter/example && fvm flutter drive --profile --driver test_driver/integration_test.dart --target integration_test/canvist_test.dart`.

### Keeping the binding cheap

UniFFI calls are not free: on Android each one costs about 25 µs through JNA (release build; the placeholder handle measured 20 ns in Rust, 120 ns through Swift, and 25 µs through Kotlin on 2026-10-18), mostly in `Structure` marshalling. The editor's API is shaped around that:

- **Coarse calls.** A keystroke is one call (`insertText`, `deleteBackward`, `press`) and one `frame()`. Text input queries return small records, never the whole document.
- **Frames only when something changed.** `frame()` returns the display list only when it differs from the last one, with the damaged rectangles, so an idle frame costs one call and no list.
- **Batched measurement.** Layout collects every segment a relayout needs and asks the platform once, and caches the answers, so typing usually measures nothing.
- **Owned byte arrays.** Collaboration methods take `Vec<u8>`, which both platforms see as their native byte arrays.

Library sizes (release, `lto = true`, stripped):

| Artifact                                   | Size              |
| ------------------------------------------ | ----------------- |
| `libcanvist_ffi.so` `arm64-v8a` / `x86_64` | 2.6 MiB / 3.0 MiB |
| JNA runtime that the Kotlin binding needs  | 162 KiB + 218 KiB |

About two thirds is code (`cargo bloat` on a host build: the standard library about 570 KiB, yrs and the Yjs binding about 450 KiB, the editor, layout, and model crates about 600 KiB); the rest is read-only data such as Unicode tables. A collaboration-free build is the obvious way to shrink it. Apps must set `abiFilters` to the ABIs we build: JNA's AAR ships `libjnidispatch.so` for seven ABIs, and without a filter the app installs on devices it cannot run on.

## Troubleshooting

- **`No available simulator`**: install a runtime with `xcodebuild -downloadPlatform iOS`, or set `CANVIST_IOS_SIMULATOR`.
- **`Unknown AVD name`**: the AVD was created in a different AVD home. Check `emulator -list-avds` and `ANDROID_AVD_HOME`.
- **`requires libraries and applications that depend on it to compile against version 37`**: install `platforms;android-37` with `sdkmanager`.
- **`The rust-toolchain.toml does not include the target`** (Flutter): add the target to `platforms/dart/rust/rust-toolchain.toml`.
