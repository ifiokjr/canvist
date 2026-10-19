# Quick start: Flutter

In this guide you add canvist's engine to a Flutter app and drive a document from your own widgets. It takes about five minutes.

**Status: engine available, editor widget coming soon.** `canvist_flutter` gives your app the full engine today: documents, typed commands, every format, and collaboration. It doesn't include the canvas editor widget yet, so you show the document with your own widgets for now. When the widget lands, the engine code you write today keeps working.

## 1. Install

You need Flutter 3.47 or newer and a [Rust toolchain](https://rustup.rs). The engine is written in Rust, and a build hook compiles it for each platform you build.

Add the package to `pubspec.yaml`. It isn't on pub.dev yet, so install it from GitHub:

```yaml
dependencies:
  canvist_flutter:
    git:
      url: https://github.com/ifiokjr/canvist
      path: platforms/flutter
```

```sh
flutter pub get
```

## 2. Load the engine and edit a document

Load the engine once, before `runApp`. Then create documents and run commands from your widgets. This app adds what you type to a document, has a button that toggles bold, and shows the result as Markdown:

```dart
import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final (:engine, :kit) = await Engine.starterKit();
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
  late final Doc _doc = widget.engine.create();
  final _draft = TextEditingController();
  String _markdown = '';

  void _addText() {
    final end = _doc.export(Format.text).length;
    _doc
      ..select(Position([0], end))
      ..insertText(_draft.text);
    _draft.clear();
    _refresh();
  }

  void _toggleBold() {
    _doc
      ..selectAll()
      ..run(widget.kit.bold.toggleBold());
    _refresh();
  }

  void _refresh() => setState(() => _markdown = _doc.export(Format.markdown));

  @override
  void dispose() {
    _draft.dispose();
    _doc.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Notes'),
        actions: [
          IconButton(
            icon: const Icon(Icons.format_bold),
            onPressed: _toggleBold,
          ),
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          children: [
            TextField(controller: _draft, onSubmitted: (_) => _addText()),
            const SizedBox(height: 16),
            SelectableText(_markdown),
          ],
        ),
      ),
    );
  }
}
```

Run it on any device:

```sh
flutter run
```

## Next steps

- [Quick start: Dart](./server-dart.md): the same engine on a Dart server.
- [Commands and typed handles](../concepts/commands.md): toolbar buttons map one-to-one onto commands.
- [Collaboration](../guides/collaboration.md): sync documents with Yjs.
