import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const CanvistExampleApp());
}

class CanvistExampleApp extends StatelessWidget {
  const CanvistExampleApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Canvist',
      theme: ThemeData(colorSchemeSeed: Colors.indigo),
      home: const EditorScreen(),
    );
  }
}

/// Appends text to a Rust-owned document and renders what Rust returns.
class EditorScreen extends StatefulWidget {
  const EditorScreen({super.key});

  @override
  State<EditorScreen> createState() => _EditorScreenState();
}

class _EditorScreenState extends State<EditorScreen> {
  final _editor = EditorHandle();
  final _draft = TextEditingController();
  String _document = '';
  String? _failure;

  @override
  void dispose() {
    _draft.dispose();
    _editor.dispose();
    super.dispose();
  }

  void _append() {
    final text = _draft.text;
    if (text.isEmpty) return;
    try {
      // The placeholder core counts offsets in Unicode scalar values.
      _editor.insertText(offset: _document.runes.length, text: text);
      setState(() {
        _document = _editor.text();
        _failure = null;
      });
      _draft.clear();
    } on OffsetOutOfBounds catch (error) {
      setState(() => _failure = 'Offset ${error.offset} is past ${error.length}');
    }
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
          ListenableBuilder(
            listenable: _draft,
            builder: (context, _) => FilledButton(
              key: const Key('append'),
              onPressed: _draft.text.isEmpty ? null : _append,
              child: const Text('Append'),
            ),
          ),
          const SizedBox(height: 24),
          Text('Document (from Rust)', style: theme.textTheme.labelLarge),
          const SizedBox(height: 8),
          Text(
            _document.isEmpty ? 'Empty' : _document,
            key: const Key('document'),
            style: theme.textTheme.bodyLarge,
          ),
          if (_failure case final failure?)
            Text(failure, style: TextStyle(color: theme.colorScheme.error)),
          const SizedBox(height: 24),
          Text(
            'Rust core ${_editor.version()}',
            style: theme.textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}
