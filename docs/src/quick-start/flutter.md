# Quick start: Flutter

In this guide you add canvist's editor to a Flutter app, with a toolbar that formats the selection. It takes about five minutes.

**Status: Available.** `CanvistEditor` is a rich-text editor painted from canvist's Rust layout and measured by Flutter's own text engine. It supports platform text input (IME composition, autocorrect, dictation), selection handles and a toolbar on touch devices, mouse and keyboard selection on desktop, undo and redo, scrolling, accessibility, and Yjs collaboration. Every change is tested on iOS and Android devices, and on macOS and Linux hosts. Windows desktop builds use the same code but aren't tested yet. Flutter web isn't supported yet; use [`@canvist/web`](./web.md) in the browser.

## 1. Install

You need Flutter 3.47 or newer and a [Rust toolchain](https://rustup.rs). The engine is written in Rust, and a build hook compiles it for each platform you build.

```sh
flutter pub add canvist_flutter
```

Before the first release reaches pub.dev, install it from GitHub instead:

```yaml
dependencies:
  canvist_flutter:
    git:
      url: https://github.com/canvist/canvist
      path: platforms/flutter
```

## 2. Show an editor with a toolbar

Load the engine once, before `runApp`. Building it with `CanvistController.keyPlatform` makes shortcuts such as `Mod-b` use Cmd on Apple devices and Ctrl elsewhere. A `CanvistController` holds the document. Toolbar buttons run typed commands from the engine's handles and light up from typed queries:

```dart
import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final (:engine, :kit) = await Engine.starterKit(
    platform: CanvistController.keyPlatform,
  );
  runApp(MaterialApp(home: NotesScreen(engine: engine, kit: kit)));
}

class NotesScreen extends StatefulWidget {
  const NotesScreen({super.key, required this.engine, required this.kit});

  final Engine engine;
  final StarterKitHandles kit;

  @override
  State<NotesScreen> createState() => _NotesScreenState();
}

class _NotesScreenState extends State<NotesScreen> {
  late final _controller = CanvistController(
    engine: widget.engine,
    doc: widget.engine.parse(Format.markdown, '# Notes\n\nStart typing.'),
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final kit = widget.kit;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Notes'),
        actions: [
          ListenableBuilder(
            listenable: _controller,
            builder: (context, _) => IconButton(
              icon: const Icon(Icons.format_bold),
              isSelected: _controller.query(kit.bold.isBold()),
              onPressed: () => _controller.run(kit.bold.toggleBold()),
            ),
          ),
          IconButton(
            icon: const Icon(Icons.undo),
            onPressed: _controller.undo,
          ),
        ],
      ),
      body: CanvistEditor(controller: _controller, autofocus: true),
    );
  }
}
```

Run it on any device:

```sh
flutter run
```

Type `##` at the start of a line for a heading, or `-` for a list. Select text and press the bold button, or Cmd-B (Ctrl-B). Export the document whenever you need it, for example `_controller.export(Format.markdown)`.

## 3. Collaborate

A controller's document is a Yjs peer. Share it, send what it produces to your provider, and apply what arrives through the controller so the editor repaints:

```dart no_run
import 'dart:typed_data';

import 'package:canvist_flutter/canvist_flutter.dart';

void connect(CanvistController controller, void Function(Uint8List) send) {
  controller.doc.share(1);
  controller.addListener(() => controller.doc.flushSync().forEach(send));
}

void onMessage(
  CanvistController controller,
  Uint8List message,
  void Function(Uint8List) send,
) {
  controller.receiveSync(message).reply.forEach(send);
}
```

Undo on a shared document reverts only this peer's changes. The example app's **Two peers** switch shows two editors syncing one document through an in-memory relay.

## Styling

`CanvistEditor` follows the app's brightness and text scale. `CanvistStyle` sets the font families (as CSS lists, such as `Inter, sans-serif`), size, and line height. `CanvistFonts` decides which Flutter fonts those names mean. Layout measures and the painter draws through the same fonts, so text always wraps where it's drawn.

## Next steps

- [Commands and typed handles](../concepts/commands.md): toolbar buttons map one-to-one onto commands.
- [Collaboration](../guides/collaboration.md): sync documents with Yjs.
- [Quick start: Dart](./server-dart.md): the same engine on a Dart server, including server rendering with `doc.toHtml`.
