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

canvist's rich-text editor for Flutter. `CanvistEditor` paints a document laid out by canvist's Rust core, with text measured by Flutter's own text engine, so lines wrap exactly where Flutter draws them. It takes the platform's text input, with IME composition, autocorrect, and dictation. Touch devices get selection handles and a toolbar, and desktop gets mouse and keyboard selection. Undo and redo, scrolling, accessibility, and Yjs collaboration are built in. The pure Dart [`canvist`](../dart) engine is re-exported, so the same code runs on your Dart server.

```sh
flutter pub add canvist_flutter
```

You need Flutter 3.47 or newer and a [Rust toolchain](https://rustup.rs): a build hook compiles the Rust core for each platform you build. It runs on iOS, Android, macOS, Linux, and Windows; Flutter web isn't supported yet.

```dart
import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Load once, before runApp. Mod is Cmd on Apple devices, Ctrl elsewhere.
  final (:engine, :kit) = await Engine.starterKit(
    platform: CanvistController.keyPlatform,
  );
  final controller = CanvistController(
    engine: engine,
    doc: engine.parse(Format.markdown, '# Hello\n\nStart typing.'),
  );

  runApp(
    MaterialApp(
      home: Scaffold(
        appBar: AppBar(
          actions: [
            IconButton(
              icon: const Icon(Icons.format_bold),
              onPressed: () => controller.run(kit.bold.toggleBold()),
            ),
          ],
        ),
        body: CanvistEditor(controller: controller, autofocus: true),
      ),
    ),
  );
}
```

Commands come from the engine's typed handles, so a toolbar can only call extensions the engine registered. `controller.query(kit.bold.isBold())` lights a button up, and the controller notifies listeners on every change. The [Flutter quick start](../../docs/src/quick-start/flutter.md) walks through a toolbar and collaboration, and [`example/`](./example) is a full editor app with a formatting toolbar and a two-peer collaboration demo.

## How it works

- **Layout** happens in Rust (`canvist_layout`), incrementally: only the blocks a change touched are laid out again, and only the viewport is measured. A `ParagraphMeasurer` answers measurement batches with `dart:ui` paragraphs, in one synchronous call per relayout.
- **Painting**: `DisplayPainter` draws the display list, and shaped text runs are cached, so a keystroke reshapes one word.
- **Input**: a delta-model `TextInputClient` edits the block holding the caret, and hardware keys run the extensions' key bindings.
- **Speed**: a keystroke on a 10,000-paragraph document reaches the next frame in a fraction of a frame. Budgets in `platforms/dart/benchmark/overhead.dart`, the widget tests, and the integration tests keep it that way.

<details>
<summary><strong>Development</strong></summary>

```sh
flutter pub get
flutter test   # widget tests and golden tests against server rendering
./test.sh      # the example app's integration tests on an iOS Simulator and an Android emulator
```

</details>
