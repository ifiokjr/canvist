/// Details, callouts, and math through the Dart API: typed handles,
/// metadata, errors, TipTap JSON, and collaboration. The shared cases in
/// `conformance/fixtures/blocks.json` cover editing and the formats.
library;

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

typedef Blocks = ({
  Engine engine,
  DetailsHandle details,
  CalloutHandle callout,
  MathematicsHandle math,
});

Future<Blocks> blocks({int client = 1}) async {
  final builder = EngineBuilder();
  builder.add(Extensions.history);
  final details = builder.add(Extensions.details);
  final callout = builder.add(Extensions.callout);
  final math = builder.add(Extensions.mathematics);
  final engine = await builder.build(client: client);

  return (engine: engine, details: details, callout: callout, math: math);
}

Matcher throwsCode(ErrorCode code) =>
    throwsA(isA<CanvistException>().having((e) => e.code, 'code', code));

/// The JSON without block ids, and without the `attrs` they leave empty.
Object? withoutIds(Object? node) => switch (node) {
  final List<Object?> list => list.map(withoutIds).toList(),
  final Map<String, Object?> map => {
    for (final MapEntry(:key, :value) in map.entries)
      if (key != 'attrs')
        key: withoutIds(value)
      else if ((value! as Map<String, Object?>).keys.any((k) => k != 'id'))
        key: {
          for (final MapEntry(:key, :value) in (value as Map).entries)
            if (key != 'id') key as String: value,
        },
  },
  _ => node,
};

void main() {
  test('the metadata describes every block command and node', () async {
    final (:engine, details: _, callout: _, math: _) = await blocks();
    final byName = {
      for (final extension in engine.metadata.cast<Map<String, Object?>>())
        extension['name']: extension,
    };
    List<Object?> names(String extension, String key) =>
        (byName[extension]![key]! as List<Object?>)
            .map((e) => (e! as Map<String, Object?>)['name'])
            .toList();

    expect(names('details', 'commands'), [
      'setDetails',
      'unsetDetails',
      'toggleDetailsOpen',
    ]);
    expect(names('details', 'queries'), ['detailsOpen']);
    expect(names('details', 'nodes'), [
      'details',
      'detailsSummary',
      'detailsContent',
    ]);
    expect(names('callout', 'commands'), ['toggleCallout', 'setCalloutType']);
    expect(names('callout', 'queries'), ['calloutType']);
    expect(names('mathematics', 'commands'), [
      'insertInlineMath',
      'insertBlockMath',
    ]);
    expect(names('mathematics', 'nodes'), ['inlineMath', 'blockMath']);
  });

  test('typed handles run, check, and query every block command', () async {
    final (:engine, :details, :callout, :math) = await blocks();
    final doc = engine.parse(Format.markdown, 'Careful')
      ..select(const Position([0], 7));

    expect(doc.query(callout.calloutType()), isNull);
    expect(doc.can(callout.setCalloutType(type: 'error')), isFalse);
    expect(doc.run(callout.toggleCallout(type: 'warning')), isTrue);
    expect(doc.can(callout.setCalloutType(type: 'error')), isTrue);
    expect(doc.run(callout.setCalloutType(type: 'error')), isTrue);
    expect(doc.query(callout.calloutType()), 'error');
    expect(doc.export(Format.markdown), '> [!CAUTION]\n> Careful');
    expect(doc.run(callout.toggleCallout()), isTrue);
    expect(doc.query(callout.calloutType()), isNull);

    expect(doc.can(details.unsetDetails()), isFalse);
    expect(doc.run(details.setDetails()), isTrue);
    expect(doc.query(details.detailsOpen()), isTrue);
    expect(doc.run(details.toggleDetailsOpen()), isTrue);
    expect(doc.query(details.detailsOpen()), isFalse);
    expect(
      doc.export(Format.markdown),
      '<details>\n<summary></summary>\n\nCareful\n\n</details>',
    );
    expect(doc.run(details.unsetDetails()), isTrue);
    expect(doc.query(details.detailsOpen()), isNull);

    expect(doc.run(math.insertInlineMath(latex: r'\pi')), isTrue);
    expect(doc.export(Format.markdown), r'$\pi$Careful');
    expect(doc.undo(), isTrue);
    expect(doc.export(Format.markdown), 'Careful');
    expect(doc.redo(), isTrue);
    expect(doc.run(math.insertBlockMath(latex: 'x^2')), isTrue);
    expect(
      doc.export(Format.markdown),
      '\$\\pi\$\n\n\$\$\nx^2\n\$\$\n\nCareful',
    );
  });

  test('invalid block arguments fail with invalidArgs', () async {
    final (:engine, details: _, :callout, math: _) = await blocks();
    final doc = engine.parse(Format.markdown, 'x');

    expect(
      () => doc.run(callout.toggleCallout(type: 'purple')),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('insertInlineMath'),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('insertBlockMath', {'latex': 2}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.queryNamed('calloutType', {'extra': true}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(doc.export(Format.markdown), 'x');
  });

  test(
    'an engine without the block extensions has none of their names',
    () async {
      final (:engine, kit: _) = await Engine.starterKit();
      final doc = engine.create();
      final other = await blocks();

      for (final name in ['setDetails', 'toggleCallout', 'insertBlockMath']) {
        expect(() => doc.runNamed(name), throwsCode(ErrorCode.unknownName));
      }
      expect(() => doc.run(other.details.setDetails()), throwsArgumentError);
    },
  );

  test('TipTap JSON blocks read and write back unchanged', () async {
    final (:engine, details: _, callout: _, math: _) = await blocks();
    const json = <String, Object?>{
      'type': 'doc',
      'content': [
        {
          'type': 'details',
          'attrs': {'open': true},
          'content': [
            {
              'type': 'detailsSummary',
              'content': [
                {'type': 'text', 'text': 'Title'},
              ],
            },
            {
              'type': 'detailsContent',
              'content': [
                {
                  'type': 'callout',
                  'attrs': {'type': 'success'},
                  'content': [
                    {
                      'type': 'paragraph',
                      'content': [
                        {'type': 'text', 'text': 'Area '},
                        {
                          'type': 'inlineMath',
                          'attrs': {'latex': r'\pi r^2'},
                        },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
        {
          'type': 'blockMath',
          'attrs': {'latex': 'E = mc^2'},
        },
      ],
    };
    final doc = engine.fromJson(json);

    expect(withoutIds(doc.toJson()), json);
    expect(
      doc.export(Format.markdown),
      '<details open>\n<summary>Title</summary>\n\n> [!TIP]\n'
      '> Area \$\\pi r^2\$\n\n</details>\n\n\$\$\nE = mc^2\n\$\$',
    );
  });

  test('peers editing blocks concurrently converge', () async {
    final first = await blocks();
    final second = await blocks(client: 2);
    final a = first.engine.parse(
      Format.markdown,
      '> [!NOTE]\n> Careful\n\n'
      '<details open>\n<summary>Sum</summary>\n\nBody\n\n</details>',
    )..share(1);
    final b = second.engine.fromYjs(a.encodeState(), clientId: 2);
    expect(b.toJson(), a.toJson());

    a
      ..select(const Position([0, 0], 0))
      ..run(first.callout.setCalloutType(type: 'warning'))
      ..select(const Position([1, 0], 0))
      ..run(first.details.toggleDetailsOpen());
    b
      ..select(const Position([0, 0], 7))
      ..insertText(' now')
      ..select(const Position([1, 1, 0], 4))
      ..run(second.math.insertInlineMath(latex: 'x'));

    final fromA = a.takeUpdates();
    b.takeUpdates().forEach(a.applyUpdate);
    fromA.forEach(b.applyUpdate);

    expect(a.toJson(), b.toJson());
    expect(
      a.export(Format.markdown),
      '> [!WARNING]\n> Careful now\n\n'
      '<details>\n<summary>Sum</summary>\n\nBody\$x\$\n\n</details>',
    );
    final c = first.engine.fromYjs(a.encodeState(), clientId: 3);
    expect(c.toJson(), a.toJson());
    expect(a.validate(), (rejectedBlocks: 0, waiting: false));
  });
}
