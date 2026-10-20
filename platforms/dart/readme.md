<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/quick-start/server-dart.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://pub.dev/packages/canvist"><img src="https://img.shields.io/pub/v/canvist?color=3B2CF5" alt="pub.dev version" /></a>
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/dart.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/dart.yml/badge.svg?branch=main" alt="dart status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist (Dart)

canvist's document model and editing engine in pure Dart, with no Flutter. Convert ProseMirror/TipTap JSON, HTML, Markdown, and text, run typed editing commands on a server, and serve Yjs collaboration from the same Rust core the editor uses. A Dart build hook compiles it, so `dart pub get` is all a project needs, plus a Rust toolchain while building.

```sh
dart pub add canvist
```

```dart
import 'package:canvist/canvist.dart';

Future<void> main() async {
  final (:engine, :kit) = await Engine.starterKit();
  final doc = engine.parse(Format.markdown, 'Hello world');

  doc.select(const Position([0], 6), const Position([0], 11));
  doc.run(kit.bold.toggleBold());
  print(doc.export(Format.html)); // <p>Hello <strong>world</strong></p>
}
```

Building a Flutter app? [`canvist_flutter`](../flutter) re-exports this package, so the same code runs on a Dart server and in your app.

## Typed extension handles

Registering an extension returns its handle, and commands and queries only exist on handles, so code can't call an extension the engine doesn't have:

```dart
final builder = EngineBuilder();
final heading = builder.add(Extensions.heading);
final engine = await builder.build();

final doc = engine.create()..run(heading.setHeading(level: 2));
doc.query(heading.headingLevel()); // int?: 2
```

Callers that choose commands at runtime use `runNamed`, `canNamed`, and `queryNamed`, which Rust checks against the extension metadata.

## Collaboration

```dart
final doc = engine.parse(Format.markdown, '# Notes')..share(1);
socket.add(doc.syncStart());                 // when a client connects
final reply = doc.receiveSync(message);      // for each message
reply.reply.forEach(socket.add);             // and relay reply.broadcast to the others
```

[`example/server.dart`](./example/server.dart) is a `dart:io` server with `POST /convert` and a y-websocket-compatible `/sync/<room>` endpoint.

A typed command costs about 2 µs and a collaborator's keystroke about 3.5 µs, at 10,000 paragraphs. `dart run benchmark/overhead.dart` reproduces the numbers.

<details>
<summary><strong>Development</strong></summary>

```sh
dart pub get
dart test                                        # conformance, API, and server tests
dart run tool/generate.dart                      # regenerate the typed handles
cargo bin flutter_rust_bridge_codegen generate   # after changing rust/src/api
```

`dart run`, `dart test`, and `dart build cli` work on macOS and Linux. Compile executables with `dart build cli`, which bundles the Rust library; `dart compile exe` can't bundle native assets.

The tests run the shared fixtures in [`conformance/`](../../conformance), which the Rust and TypeScript packages run too.

</details>
