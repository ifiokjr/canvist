# canvist (Dart)

canvist's document model and editing engine in pure Dart, with no Flutter: convert ProseMirror/TipTap JSON, HTML, Markdown, and text, run typed editing commands on a server, and serve Yjs collaboration. It is the Rust core, [`canvist_headless`](../../crates/canvist_headless), bound through `flutter_rust_bridge` and compiled by a Dart build hook, so `dart pub get` is all a project needs (plus a Rust toolchain while building).

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

[`canvist_flutter`](../flutter) builds on this package and re-exports it, so the same code runs on a Dart server and in a Flutter app.

## Typed extension handles

Registering an extension returns its handle, and commands and queries only exist on handles, so code cannot call an extension the engine does not have (architecture Decision 8). The handles are generated from the same metadata the Rust commands check their arguments against (`tool/generate.dart`):

```dart
final builder = EngineBuilder();
final heading = builder.add(Extensions.heading);
final engine = await builder.build();

final doc = engine.create()..run(heading.setHeading(level: 2));
doc.query(heading.headingLevel()); // int?: 2
```

A handle from one engine is refused by another engine's documents. Callers that choose commands at runtime use `runNamed`, `canNamed`, and `queryNamed`, which Rust checks against the metadata.

## Positions and errors

A `Position` is a path from the root (`[1, 0, 0]` is the paragraph in the first item of a list that is the second block) and an offset in UTF-16 code units, the unit of Dart string indexes. Every failure is a `CanvistException` whose `code` is an `ErrorCode` shared with Rust and TypeScript.

## Collaboration

```dart
final doc = engine.parse(Format.markdown, '# Notes')..share(1);
socket.add(doc.syncStart());                 // when a client connects
final reply = doc.receiveSync(message);      // for each message
reply.reply.forEach(socket.add);             // and relay reply.broadcast to the others
doc.validate();                              // (rejectedBlocks: 0, waiting: false)
```

`example/server.dart` is a `dart:io` server with `POST /convert` and a y-websocket-compatible `/sync/<room>` endpoint, matching the TypeScript example. `test/server_test.dart` drives it with two peers over real WebSockets.

```sh
dart run example/server.dart
curl -X POST 'localhost:8787/convert?from=markdown&to=json' -d '# Hello **world**'
```

## Running and compiling

`dart run`, `dart test`, and `dart build cli` all work on macOS and Linux. `dart build cli` is how Dart compiles an executable for a package with native assets: it writes `bundle/bin/<name>` and the Rust library in `bundle/lib/`. `dart compile exe` cannot bundle native assets, so a binary it produces fails to find the library.

flutter_rust_bridge's own loader opens the library by file name, which works inside a Flutter app bundle but not where `dart run` keeps native assets. `lib/src/load.dart` resolves one `@Native` symbol bound to the asset, so the Dart VM loads the library through its native-assets mapping, then asks the dynamic linker (`dladdr`) for its path. iOS and Android keep flutter_rust_bridge's loader.

## Performance

`dart run benchmark/overhead.dart` measures the same operations as the Rust budgets (Apple silicon, JIT, 10,000 paragraphs):

| Operation                      | Rust    | Dart   | Budget   |
| ------------------------------ | ------- | ------ | -------- |
| Run a typed command            | 0.21 µs | 2.0 µs | < 10 µs  |
| Answer a query                 | –       | 1.3 µs | < 10 µs  |
| Type or delete a character     | 0.68 µs | 1.8 µs | < 10 µs  |
| Apply one Yjs keystroke update | 2.3 µs  | 3.5 µs | < 30 µs  |
| Load 10k paragraphs (Markdown) | 10 ms   | 13 ms  | < 100 ms |
| Export 10k paragraphs as JSON  | 19 ms   | 21 ms  | < 100 ms |

## Development

```sh
dart pub get
dart test                                # conformance, API, and server tests
dart run tool/generate.dart              # regenerate the typed handles
cargo bin flutter_rust_bridge_codegen generate   # after changing rust/src/api
```

The tests run the shared fixtures in [`conformance/`](../../conformance), which the Rust and TypeScript packages run too.
