/// Ink and kinetic text through the Dart API: typed handles and records,
/// argument errors with stable codes, JSON and HTML round trips, and
/// drawings shared between two documents over Yjs.
library;

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

Matcher throwsCode(ErrorCode code) =>
    throwsA(isA<CanvistException>().having((e) => e.code, 'code', code));

void main() {
  test('ink draws, undoes, and clears through its handle', () async {
    final builder = EngineBuilder();
    final ink = builder.add(Extensions.ink);
    final engine = await builder.build();
    final doc = engine.create();

    expect(doc.query(ink.inks()), isEmpty);
    expect(doc.can(ink.insertInk(height: 250)), isTrue);
    expect(doc.run(ink.insertInk(height: 250)), isTrue);
    expect(
      doc.run(
        ink.addInkStroke(
          ink: 0,
          points: [1.04, 2.06, 1200, -5],
          color: '#ABCDEF',
          width: 3.3,
        ),
      ),
      isTrue,
    );
    expect(doc.run(ink.addInkStroke(ink: 0, points: [5, 5])), isTrue);
    expect(doc.query(ink.inks()), [(height: 250, strokes: 2)]);

    final attrs =
        ((doc.toJson()['content']! as List<Object?>).first!
                as Map<String, Object?>)['attrs']!
            as Map<String, Object?>;
    expect((attrs['strokes']! as List<Object?>).first, {
      'points': [1.0, 2.1, 1000.0, 0.0],
      'color': '#abcdef',
      'width': 3.3,
    });

    expect(doc.run(ink.undoInkStroke(ink: 0)), isTrue);
    expect(doc.query(ink.inks()).single.strokes, 1);
    expect(doc.undo(), isTrue);
    expect(doc.query(ink.inks()).single.strokes, 2);
    expect(doc.run(ink.clearInk(ink: 0)), isTrue);
    expect(doc.can(ink.clearInk(ink: 0)), isFalse);
    expect(doc.run(ink.clearInk(ink: 3)), isFalse);
    expect(doc.undo(), isTrue);
    expect(doc.query(ink.inks()).single.strokes, 2);
  });

  test('ink refuses malformed strokes with stable codes', () async {
    final builder = EngineBuilder();
    final ink = builder.add(Extensions.ink);
    final engine = await builder.build();
    final doc = engine.create()..run(ink.insertInk());

    for (final command in [
      ink.insertInk(height: 0),
      ink.insertInk(height: 4001),
      ink.addInkStroke(ink: 0, points: []),
      ink.addInkStroke(ink: 0, points: [1, 2, 3]),
      ink.addInkStroke(ink: 0, points: [1, 2], color: 'red'),
      ink.addInkStroke(ink: 0, points: [1, 2], width: 0),
      ink.addInkStroke(ink: -1, points: [1, 2]),
      ink.clearInk(ink: -1),
    ]) {
      expect(() => doc.run(command), throwsCode(ErrorCode.invalidArgs));
    }
    expect(
      () => doc.runNamed('addInkStroke', {
        'ink': 0,
        'points': [1, 'x'],
      }),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(doc.query(ink.inks()), [(height: 400, strokes: 0)]);
  });

  test('drawings round trip through JSON, HTML, and Markdown', () async {
    final builder = EngineBuilder();
    final ink = builder.add(Extensions.ink);
    final engine = await builder.build();
    final doc = engine.fromJson({
      'type': 'doc',
      'content': [
        {
          'type': 'ink',
          'attrs': {
            'height': 120,
            'strokes': [
              {
                'points': [10, 20, 30, 40],
                'color': '#ff0000',
                'width': 8,
              },
            ],
          },
        },
      ],
    });

    final html = doc.export(Format.html);
    expect(html, contains('role="img" aria-label="Drawing"'));
    expect(html, contains('<path d="M10 20L30 40"'));
    expect(engine.parse(Format.html, html).toJson(), doc.toJson());
    expect(
      engine.parse(Format.markdown, doc.export(Format.markdown)).toJson(),
      doc.toJson(),
    );
    expect(engine.restore(doc.snapshot()).query(ink.inks()), [
      (height: 120, strokes: 1),
    ]);
  });

  test('kinetic text sets, toggles, and reads every effect', () async {
    final builder = EngineBuilder();
    final kinetic = builder.add(Extensions.kinetic);
    final engine = await builder.build();
    final doc = engine.parse(Format.text, 'Say hello')
      ..select(const Position([0], 4), const Position([0], 9));

    expect(doc.query(kinetic.kineticEffect()), isNull);
    for (final effect in ['wave', 'shake', 'pulse', 'rainbow']) {
      expect(doc.run(kinetic.setKinetic(effect: effect)), isTrue);
      expect(doc.query(kinetic.kineticEffect()), effect);
    }
    expect(doc.run(kinetic.toggleKinetic(effect: 'wave')), isTrue);
    expect(doc.query(kinetic.kineticEffect()), 'wave');
    expect(doc.run(kinetic.toggleKinetic(effect: 'wave')), isTrue);
    expect(doc.query(kinetic.kineticEffect()), isNull);
    expect(doc.undo(), isTrue);
    expect(
      doc.export(Format.html),
      '<p>Say <span data-kinetic="wave">hello</span></p>',
    );
    expect(doc.export(Format.markdown), 'Say hello');
    expect(doc.run(kinetic.unsetKinetic()), isTrue);
    expect(
      () => doc.run(kinetic.setKinetic(effect: 'spin')),
      throwsCode(ErrorCode.invalidArgs),
    );

    final pasted = engine.parse(
      Format.html,
      '<p><span data-kinetic="pulse">beat</span></p>',
    )..selectAll();
    expect(pasted.query(kinetic.kineticEffect()), 'pulse');
  });

  test('strokes on two documents converge over Yjs', () async {
    final builder = EngineBuilder();
    final ink = builder.add(Extensions.ink);
    final engine = await builder.build();
    final first = engine.fromJson({
      'type': 'doc',
      'content': [
        {'type': 'ink'},
        {'type': 'ink'},
      ],
    })..share(1);
    final second = engine.fromYjs(first.encodeState(), clientId: 2);
    first.takeUpdates();

    first.run(ink.addInkStroke(ink: 0, points: [1, 2]));
    second.run(ink.addInkStroke(ink: 1, points: [3, 4]));
    final fromFirst = first.takeUpdates();
    second.takeUpdates().forEach(first.applyUpdate);
    fromFirst.forEach(second.applyUpdate);

    expect(first.toJson(), second.toJson());
    expect(second.query(ink.inks()), [
      (height: 400, strokes: 1),
      (height: 400, strokes: 1),
    ]);
    expect(second.validate(), (rejectedBlocks: 0, waiting: false));
  });
}
