<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/flutter.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://pub.dev/packages/canvist_flutter"><img src="https://img.shields.io/pub/v/canvist_flutter?color=3B2CF5" alt="pub.dev version" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/dart.yml"><img src="https://github.com/canvist/canvist/actions/workflows/dart.yml/badge.svg?branch=main" alt="dart status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_flutter

canvist for Flutter. It re-exports the pure Dart [`canvist`](../dart) package, so your app gets the full engine today: documents, typed commands, every format, and Yjs collaboration. The canvas editor widget is coming soon, and your engine code stays the same when it arrives.

```sh
flutter pub add canvist_flutter
```

You need Flutter 3.47 or newer and a [Rust toolchain](https://rustup.rs): a build hook compiles the Rust core for each platform you build.

```dart
import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final (:engine, :kit) = await Engine.starterKit(); // load once, before runApp

  final doc = engine.parse(Format.markdown, 'Hello world');
  doc.select(const Position([0], 6), const Position([0], 11));
  doc.run(kit.bold.toggleBold());

  runApp(MaterialApp(home: Text(doc.export(Format.markdown)))); // Hello **world**
}
```

The [Flutter quick start](../../docs/src/quick-start/flutter.md) builds a small notes app with a bold button.

<details>
<summary><strong>Development</strong></summary>

```sh
flutter pub get
./test.sh    # the example app's integration tests on an iOS Simulator and an Android emulator
```

</details>
