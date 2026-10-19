import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

Future<void> main() async {
  final (:engine, :kit) = await Engine.starterKit();
  runApp(CanvistExampleApp(engine: engine, kit: kit));
}

class CanvistExampleApp extends StatelessWidget {
  const CanvistExampleApp({super.key, required this.engine, required this.kit});

  final Engine engine;
  final StarterKitHandles kit;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Canvist',
      theme: ThemeData(colorSchemeSeed: Colors.indigo),
      home: EditorScreen(engine: engine, kit: kit),
    );
  }
}

/// Appends text to a Rust-owned canvist document and shows it as Markdown.
class EditorScreen extends StatefulWidget {
  const EditorScreen({super.key, required this.engine, required this.kit});

  final Engine engine;
  final StarterKitHandles kit;

  @override
  State<EditorScreen> createState() => _EditorScreenState();
}

class _EditorScreenState extends State<EditorScreen> {
  late final Doc _doc = widget.engine.create();
  final _draft = TextEditingController();
  String _markdown = '';

  @override
  void dispose() {
    _draft.dispose();
    _doc.dispose();
    super.dispose();
  }

  void _append() {
    final text = _draft.text;

    if (text.isEmpty) {
      return;
    }

    final end = _doc.export(Format.text).length;
    _doc
      ..select(Position([0], end))
      ..insertText(text);
    setState(() => _markdown = _doc.export(Format.markdown));
    _draft.clear();
  }

  void _toggleBold() {
    _doc
      ..selectAll()
      ..run(widget.kit.bold.toggleBold());
    setState(() => _markdown = _doc.export(Format.markdown));
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(title: const Text('Canvist')),
      body: ListView(
        padding: const EdgeInsets.all(24),
        children: [
          TextField(
            key: const Key('draft'),
            controller: _draft,
            decoration: const InputDecoration(
              labelText: 'Text',
              border: OutlineInputBorder(),
            ),
            onSubmitted: (_) => _append(),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              ListenableBuilder(
                listenable: _draft,
                builder: (context, _) => FilledButton(
                  key: const Key('append'),
                  onPressed: _draft.text.isEmpty ? null : _append,
                  child: const Text('Append'),
                ),
              ),
              const SizedBox(width: 12),
              OutlinedButton(
                key: const Key('bold'),
                onPressed: _markdown.isEmpty ? null : _toggleBold,
                child: const Text('Bold'),
              ),
            ],
          ),
          const SizedBox(height: 24),
          Text('Document as Markdown (from Rust)', style: theme.textTheme.labelLarge),
          const SizedBox(height: 8),
          Text(
            _markdown.isEmpty ? 'Empty' : _markdown,
            key: const Key('document'),
            style: theme.textTheme.bodyLarge,
          ),
        ],
      ),
    );
  }
}
