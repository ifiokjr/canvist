/// Runs the language-neutral fixtures in `conformance/fixtures` against the
/// Dart API. See `conformance/readme.md` for the format.
library;

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

typedef Fields = Map<String, Object?>;

final _fixtures = Directory('../../conformance/fixtures');

/// The optional features this binding has.
const _features = {'collab', 'extensions'};

/// Engines by the extension list a case adds to the StarterKit.
final _engines = <String, Engine>{};

/// The StarterKit plus the `extensions` [testCase] lists, by name.
Future<Engine> _engineFor(Fields testCase) async {
  final names = (testCase['extensions'] as List<Object?>? ?? const [])
      .cast<String>();
  final key = names.join(',');
  final cached = _engines[key];

  if (cached != null) {
    return cached;
  }

  final builder = EngineBuilder();

  for (final extension in [
    ...Extensions.starterKit,
    ...Extensions.all.where((e) => names.contains(e.name)),
  ]) {
    builder.add(extension);
  }

  return _engines[key] = await builder.build();
}

void main() {
  final files =
      _fixtures
          .listSync()
          .whereType<File>()
          .where((file) => file.path.endsWith('.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));

  for (final file in files) {
    final fixture = jsonDecode(file.readAsStringSync()) as Fields;
    final name = file.uri.pathSegments.last;

    for (final testCase
        in (fixture['cases']! as List<Object?>).cast<Fields>()) {
      final requires = (testCase['requires'] as List<Object?>? ?? const []);
      final skip = requires.every(_features.contains)
          ? null
          : 'needs $requires';
      test(
        '$name: ${testCase['name']}',
        () async => _runCase(await _engineFor(testCase), testCase),
        skip: skip,
      );
    }
  }
}

void _runCase(Engine engine, Fields testCase) {
  final input = testCase['input'] as Fields? ?? const {};
  final error = input['error'];

  if (error is String) {
    _expectError(() => _load(engine, input), error);

    return;
  }

  var doc = _load(engine, input);

  for (final step
      in (testCase['steps'] as List<Object?>? ?? const []).cast<Fields>()) {
    doc = _runStep(engine, doc, step);
  }

  _check(doc, testCase['expect'] as Fields? ?? const {});
}

Doc _load(Engine engine, Fields input) => switch (input['format']) {
  null => engine.create(),
  'json' => engine.fromJson(input['content']! as Fields),
  'yjs' => engine.fromYjs(
    _base64(input['content']),
    clientId: (input['clientId']! as num).toInt(),
  ),
  final String format => engine.parse(
    Format.values.byName(format),
    input['content']! as String,
  ),
  final other => throw FormatException('unknown input format $other'),
};

/// Run a step, returning the document later steps use.
Doc _runStep(Engine engine, Doc doc, Fields step) {
  final error = step['error'];

  if (error is String) {
    final before = doc.export(Format.json);
    _expectError(() => _apply(doc, step), error);
    expect(
      doc.export(Format.json),
      before,
      reason: 'a failed step changes nothing',
    );

    return doc;
  }

  switch (step['op']) {
    case 'expect':
      _check(doc, step);

      return doc;
    case 'snapshotRoundTrip':
      return engine.restore(jsonEncode(jsonDecode(doc.snapshot())));
    case 'rejoin':
      return engine.fromYjs(
        doc.encodeState(),
        clientId: (step['clientId']! as num).toInt(),
      );
  }

  final outcome = _apply(doc, step);
  final expected = _expectedOutcome(step);

  if (expected != null) {
    expect(outcome, expected, reason: '${step['op']} outcome');
  }

  return doc;
}

/// Where a step declares the outcome it expects.
final _outcomes = <String, Object? Function(Fields)>{
  'receiveSync': (step) => [step['reply'], step['broadcast'], step['changed']],
  'validate': (step) => [step['rejectedBlocks'], step['waiting']],
  'takeUpdates': (step) => step['count'],
  'can': (step) => step['expect'],
  'query': (step) => step['expect'],
  'applyUpdate': (step) => step['changed'],
};

Object? _expectedOutcome(Fields step) {
  final outcome = _outcomes[step['op']];

  return outcome == null ? step['applied'] : outcome(step);
}

Fields _args(Fields step) => step['args'] as Fields? ?? const {};

String _text(Fields step) => step['text'] as String? ?? '';

/// Every operation, each returning what it reports.
final _operations = <String, Object? Function(Doc, Fields)>{
  'select': (doc, step) {
    final anchor = _position(step['anchor']);
    doc.select(anchor, step['head'] == null ? anchor : _position(step['head']));

    return true;
  },
  'selectAll': (doc, step) {
    doc.selectAll();

    return true;
  },
  'insertText': (doc, step) => doc.insertText(_text(step)),
  'typeText': (doc, step) => doc.typeText(_text(step)),
  'deleteSelection': (doc, step) => doc.deleteSelection(),
  'press': (doc, step) => doc.press(step['key']! as String),
  'paste': (doc, step) => doc.paste(
    Format.values.byName(step['format']! as String),
    step['content']! as String,
  ),
  'run': (doc, step) => doc.runNamed(step['command']! as String, _args(step)),
  'can': (doc, step) => doc.canNamed(step['command']! as String, _args(step)),
  'query': (doc, step) => doc.queryNamed(step['query']! as String, _args(step)),
  'undo': (doc, step) => doc.undo(),
  'redo': (doc, step) => doc.redo(),
  'share': (doc, step) {
    doc.share((step['clientId']! as num).toInt());

    return true;
  },
  'applyUpdate': (doc, step) => doc.applyUpdate(_base64(step['update'])),
  'takeUpdates': (doc, step) => doc.takeUpdates().length,
  'validate': (doc, step) {
    final validation = doc.validate();

    return [validation.rejectedBlocks, validation.waiting];
  },
  'receiveSync': (doc, step) {
    final sync = doc.receiveSync(_base64(step['message']));

    return [sync.reply.length, sync.broadcast.length, sync.changed];
  },
};

Object? _apply(Doc doc, Fields step) {
  final operation = _operations[step['op']];

  if (operation == null) {
    throw ArgumentError.value(step['op'], 'op', 'unknown operation');
  }

  return operation(doc, step);
}

void _expectError(Object? Function() run, String code) {
  expect(
    run,
    throwsA(
      isA<CanvistException>().having((error) => error.code.name, 'code', code),
    ),
  );
}

void _check(Doc doc, Fields expect_) {
  for (final format in [Format.html, Format.markdown, Format.text]) {
    if (expect_.containsKey(format.name)) {
      expect(doc.export(format), expect_[format.name], reason: format.name);
    }
  }

  if (expect_.containsKey('json')) {
    expect(_stripIds(doc.toJson()), _stripIds(expect_['json']), reason: 'json');
  }

  if (expect_.containsKey('selection')) {
    final selection = doc.selection;
    final actual = selection == null
        ? null
        : {
            'anchor': _positionJson(selection.anchor),
            'head': _positionJson(selection.head),
          };
    expect(actual, expect_['selection'], reason: 'selection');
  }
}

/// Remove `id` from every `attrs` object, then every `attrs` left empty.
Object? _stripIds(Object? json) {
  if (json is List<Object?>) {
    return json.map(_stripIds).toList();
  }

  if (json is! Fields) {
    return json;
  }

  final fields = {
    for (final entry in json.entries) entry.key: _stripIds(entry.value),
  };
  final attrs = fields['attrs'];

  if (attrs is Fields) {
    final remaining = {...attrs}..remove('id');
    remaining.isEmpty ? fields.remove('attrs') : fields['attrs'] = remaining;
  }

  return fields;
}

Position _position(Object? json) {
  final fields = json! as Fields;

  return Position(
    (fields['path']! as List<Object?>).cast<int>(),
    (fields['offset']! as num).toInt(),
  );
}

Fields _positionJson(Position position) => {
  'path': position.path,
  'offset': position.offset,
};

Uint8List _base64(Object? json) => base64Decode(json! as String);
