import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

import 'peers.dart';
import 'toolbar.dart';

/// The document the app opens with.
const welcome = '''# Welcome to canvist

This editor is painted from a **Rust** layout: every line you see was broken by canvist's core and measured by Flutter's own text engine.

## Try it

- Select text and make it **bold**, *italic*, or `code`
- Type `## ` at the start of a line for a heading
- Start a line with `- [ ] ` for a task

Your list:

- [x] Lay out text in Rust
- [ ] Write something of your own

> Turn on *Two peers* to edit one document from two places at once.''';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final (:engine, :kit) = await Engine.starterKit(
    platform: CanvistController.keyPlatform,
  );
  runApp(CanvistExampleApp(engine: engine, kit: kit));
}

class CanvistExampleApp extends StatelessWidget {
  const CanvistExampleApp({
    super.key,
    required this.engine,
    required this.kit,
    this.markdown = welcome,
  });

  final Engine engine;
  final StarterKitHandles kit;

  /// The document to open.
  final String markdown;

  @override
  Widget build(BuildContext context) {
    const seed = Color(0xFF3B2CF5);

    return MaterialApp(
      title: 'canvist',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(colorSchemeSeed: seed),
      darkTheme: ThemeData(colorSchemeSeed: seed, brightness: Brightness.dark),
      home: EditorScreen(engine: engine, kit: kit, markdown: markdown),
    );
  }
}

/// One document in an editor with a toolbar, or shared by two peers.
class EditorScreen extends StatefulWidget {
  const EditorScreen({
    super.key,
    required this.engine,
    required this.kit,
    required this.markdown,
  });

  final Engine engine;
  final StarterKitHandles kit;
  final String markdown;

  @override
  State<EditorScreen> createState() => _EditorScreenState();
}

class _EditorScreenState extends State<EditorScreen> {
  late final CanvistController _controller = CanvistController(
    engine: widget.engine,
    doc: widget.engine.parse(Format.markdown, widget.markdown),
  );
  Peers? _peers;

  @override
  void dispose() {
    _peers?.dispose();
    _controller
      ..dispose()
      ..doc.dispose();
    super.dispose();
  }

  void _togglePeers(bool on) {
    setState(() {
      if (on) {
        _peers = Peers.join(_controller);
      } else {
        _peers?.dispose();
        _peers = null;
      }
    });
  }

  void _showMarkdown() {
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (context) => SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
        child: SelectableText(
          _controller.export(Format.markdown),
          key: const Key('markdown'),
          style: const TextStyle(fontFamily: 'monospace', height: 1.5),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final peers = _peers;
    final colors = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: const Text('canvist'),
        actions: [
          const Text('Two peers'),
          Switch(
            key: const Key('peers'),
            value: peers != null,
            onChanged: _togglePeers,
          ),
          IconButton(
            key: const Key('export'),
            tooltip: 'Show Markdown',
            icon: const Icon(Icons.data_object),
            onPressed: _showMarkdown,
          ),
        ],
      ),
      body: SafeArea(
        child: Column(
          children: [
            EditorToolbar(controller: _controller, kit: widget.kit),
            const Divider(height: 1),
            Expanded(
              child: peers == null
                  ? CanvistEditor(
                      key: const Key('editor'),
                      controller: _controller,
                      autofocus: true,
                      padding: const EdgeInsets.all(24),
                      semanticsLabel: 'Document',
                    )
                  : PeersView(
                      peers: peers,
                      you: colors.primary,
                      peer: colors.tertiary,
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
