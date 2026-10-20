# Quick start: Dart

In this guide you add canvist to a Dart project, then load, edit, and export a document with typed commands. It takes about five minutes. The package is pure Dart, with no Flutter, so it runs under `dart run`, `dart test`, and in compiled server binaries.

**Status: available.** The `canvist` package works today. It isn't on pub.dev yet, so install it from GitHub.

## 1. Install

You need Dart 3.13 or newer and a [Rust toolchain](https://rustup.rs). canvist's engine is written in Rust, and a Dart build hook compiles it the first time you run your app.

Add canvist to `pubspec.yaml`:

```yaml
dependencies:
  canvist:
    git:
      url: https://github.com/canvist/canvist
      path: platforms/dart
```

Then fetch it:

```sh
dart pub get
```

## 2. Load, edit, and export a document

Replace `bin/main.dart` with this:

```dart
import 'package:canvist/canvist.dart';

Future<void> main() async {
  // An engine holds your extensions. `kit` has a typed handle for each one.
  final (:engine, :kit) = await Engine.starterKit();
  final doc = engine.parse(Format.markdown, '# Plan\n\nShip the docs');

  // Select "docs" (block 1, characters 9 to 13) and make it bold.
  doc.select(const Position([1], 9), const Position([1], 13));
  doc.run(kit.bold.toggleBold());

  print(doc.query(kit.bold.isBold()));
  // true
  print(doc.export(Format.html));
  // <h1>Plan</h1><p>Ship the <strong>docs</strong></p>

  doc.dispose();
}
```

Run it. The first run compiles the Rust engine, which takes a minute or two:

```sh
dart run
```

## 3. Pick only the extensions you need

Register extensions on an `EngineBuilder`. Each call returns the extension's handle, and commands only exist on handles, so you can't call an extension you didn't register:

```dart
import 'package:canvist/canvist.dart';

Future<void> main() async {
  final builder = EngineBuilder();
  final heading = builder.add(Extensions.heading);
  final engine = await builder.build();

  final doc = engine.create()..run(heading.setHeading(level: 2));
  print(doc.query(heading.headingLevel()));
  // 2

  doc.dispose();
  engine.dispose();
}
```

## Compile a server binary

The engine is a native library, so build executables with `dart build cli`, which bundles it. `dart compile exe` can't bundle native libraries, and its binaries fail at startup.

```sh
dart build cli
```

## Next steps

- [Flutter](./flutter.md): the same package, inside a Flutter app.
- [Collaboration](../guides/collaboration.md): serve y-websocket clients from `dart:io`.
- [Import and export](../guides/import-export.md) JSON, HTML, Markdown, and text.
