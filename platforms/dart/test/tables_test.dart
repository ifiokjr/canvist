/// Tables through the Dart API: the typed handle and its record query,
/// metadata, errors, TipTap JSON with merged cells, and collaboration. The
/// shared cases in `conformance/fixtures/tables.json` cover editing and the
/// formats.
library;

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

Future<({Engine engine, TableHandle table})> tables({int client = 1}) async {
  final builder = EngineBuilder();
  builder.add(Extensions.history);
  final table = builder.add(Extensions.table);
  final engine = await builder.build(client: client);

  return (engine: engine, table: table);
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

Map<String, Object?> cell(
  String type,
  String text, {
  int colspan = 1,
  int rowspan = 1,
  List<int>? colwidth,
}) => {
  'type': type,
  'attrs': {'colspan': colspan, 'colwidth': colwidth, 'rowspan': rowspan},
  'content': [
    {
      'type': 'paragraph',
      'content': [
        {'type': 'text', 'text': text},
      ],
    },
  ],
};

void main() {
  test('the metadata describes every table command, query, and node', () async {
    final (:engine, table: _) = await tables();
    final extension = engine.metadata.cast<Map<String, Object?>>().firstWhere(
      (extension) => extension['name'] == 'table',
    );
    List<Object?> names(String key) => (extension[key]! as List<Object?>)
        .map((e) => (e! as Map<String, Object?>)['name'])
        .toList();

    expect(names('commands'), hasLength(18));
    expect(
      names('commands'),
      containsAll(['insertTable', 'mergeOrSplit', 'goToPreviousCell']),
    );
    expect(names('queries'), ['tableCell']);
    expect(names('nodes'), ['table', 'tableRow', 'tableCell', 'tableHeader']);
  });

  test('typed handles run, check, and query table commands', () async {
    final (:engine, :table) = await tables();
    final doc = engine.create();

    for (final command in [
      table.addRowAfter(),
      table.deleteColumn(),
      table.mergeCells(),
      table.goToNextCell(),
      table.fixTables(),
      table.setCellAttribute(name: 'align', value: 'left'),
    ]) {
      expect(doc.can(command), isFalse, reason: command.name);
    }

    expect(doc.query(table.tableCell()), isNull);
    expect(
      doc.run(table.insertTable(rows: 2, cols: 2, withHeaderRow: false)),
      isTrue,
    );
    expect(doc.query(table.tableCell()), (
      row: 0,
      column: 0,
      rowspan: 1,
      colspan: 1,
      header: false,
      rows: 2,
      columns: 2,
    ));

    doc
      ..typeText('a')
      ..press('Tab')
      ..typeText('b')
      ..select(
        const Position([0, 0, 0, 0], 0),
        const Position([0, 0, 1, 0], 0),
      );
    expect(doc.can(table.mergeCells()), isTrue);
    expect(doc.run(table.mergeOrSplit()), isTrue);
    expect(doc.query(table.tableCell())?.colspan, 2);
    expect(doc.run(table.toggleHeaderRow()), isTrue);
    expect(doc.query(table.tableCell())?.header, isTrue);
    expect(
      doc.run(table.setCellAttribute(name: 'align', value: 'right')),
      isTrue,
    );
    expect(doc.run(table.setCellAttribute(name: 'align', value: null)), isTrue);
    expect(doc.run(table.mergeOrSplit()), isTrue);
    expect(doc.run(table.addColumnAfter()), isTrue);
    expect(doc.run(table.addRowBefore()), isTrue);
    expect(doc.query(table.tableCell())?.rows, 3);
    expect(doc.run(table.deleteRow()), isTrue);
    expect(doc.run(table.toggleHeaderColumn()), isTrue);
    expect(doc.run(table.toggleHeaderCell()), isTrue);
    expect(doc.run(table.goToNextCell()), isTrue);
    expect(doc.run(table.goToPreviousCell()), isTrue);
    expect(doc.query(table.tableCell())?.columns, 3);

    expect(doc.undo(), isTrue);
    expect(doc.redo(), isTrue);
    expect(doc.run(table.deleteTable()), isTrue);
    expect(doc.query(table.tableCell()), isNull);
  });

  test('invalid table arguments fail with invalidArgs', () async {
    final (:engine, :table) = await tables();
    final doc = engine.create()..run(table.insertTable(rows: 1, cols: 1));
    final before = doc.export(Format.json);

    expect(
      () => doc.run(table.insertTable(rows: 0)),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.run(table.insertTable(cols: 1001)),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('insertTable', {'withHeaderRow': 1}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('setCellAttribute', {'name': 'align'}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('addRowAfter', {'count': 2}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(doc.export(Format.json), before);
  });

  test('an engine without the table extension has none of its names', () async {
    final (:engine, kit: _) = await Engine.starterKit();
    final doc = engine.create();
    final other = await tables();

    for (final name in ['insertTable', 'addRowAfter', 'mergeCells']) {
      expect(() => doc.runNamed(name), throwsCode(ErrorCode.unknownName));
    }
    expect(
      () => doc.queryNamed('tableCell'),
      throwsCode(ErrorCode.unknownName),
    );
    expect(() => doc.run(other.table.insertTable()), throwsArgumentError);
  });

  test(
    'TipTap JSON with merged cells reads and writes back unchanged',
    () async {
      final (:engine, :table) = await tables();
      final json = <String, Object?>{
        'type': 'doc',
        'content': [
          {
            'type': 'table',
            'content': [
              {
                'type': 'tableRow',
                'content': [
                  cell('tableHeader', 'Wide', colspan: 2, colwidth: [120, 0]),
                  cell('tableHeader', 'C'),
                ],
              },
              {
                'type': 'tableRow',
                'content': [
                  cell('tableCell', 'Tall', rowspan: 2),
                  cell('tableCell', 'b'),
                  cell('tableCell', 'c'),
                ],
              },
              {
                'type': 'tableRow',
                'content': [cell('tableCell', 'e'), cell('tableCell', 'f')],
              },
            ],
          },
        ],
      };
      final doc = engine.fromJson(json);

      expect(withoutIds(doc.toJson()), json);
      doc.select(const Position([0, 2, 1, 0], 0));
      expect(doc.query(table.tableCell()), (
        row: 2,
        column: 2,
        rowspan: 1,
        colspan: 1,
        header: false,
        rows: 3,
        columns: 3,
      ));
    },
  );

  test('peers changing a table while others type converge', () async {
    final first = await tables();
    final second = await tables(client: 2);
    final a = first.engine.parse(
      Format.markdown,
      '| a | b |\n| --- | --- |\n| c | d |',
    )..share(1);
    final b = second.engine.fromYjs(a.encodeState(), clientId: 2);
    expect(b.toJson(), a.toJson());

    a
      ..select(const Position([0, 0, 0, 0], 0))
      ..run(first.table.addRowAfter())
      ..run(first.table.toggleHeaderColumn());
    b
      ..select(const Position([0, 1, 1, 0], 1))
      ..insertText('!')
      ..run(second.table.setCellAttribute(name: 'align', value: 'right'));

    final fromA = a.takeUpdates();
    b.takeUpdates().forEach(a.applyUpdate);
    fromA.forEach(b.applyUpdate);

    expect(a.toJson(), b.toJson());
    final c = first.engine.fromYjs(a.encodeState(), clientId: 3);
    expect(c.toJson(), a.toJson());
    expect(a.validate(), (rejectedBlocks: 0, waiting: false));
    expect(
      a.export(Format.html),
      contains('<td><p>d!</p></td>'),
      reason: 'the typing survives the new row',
    );
    final rows = ((a.toJson()['content']! as List).first! as Map)['content']!;
    final lastRow = ((rows as List).last! as Map)['content']! as List;
    expect(((lastRow.last! as Map)['attrs']! as Map)['align'], 'right');
  });
}
