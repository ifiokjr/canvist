/// The document tools through the Dart API: typed handles and records for
/// find and replace, the table of contents, comments, block moves, and
/// clearing formatting, their argument errors, and comments shared between
/// two documents over Yjs.
library;

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

Matcher throwsCode(ErrorCode code) =>
    throwsA(isA<CanvistException>().having((e) => e.code, 'code', code));

void main() {
  findsAndReplaces();
  listsTheOutline();
  tracksComments();
  movesAndClearsBlocks();
  reportsErrors();
  describesTheMetadata();
}

/// Find and replace answers a typed record and replaces in one step.
void findsAndReplaces() {
  test('find and replace answers a typed record and replaces', () async {
    final builder = EngineBuilder();
    final find = builder.add(Extensions.findAndReplace);
    final bold = builder.add(Extensions.bold);
    final engine = await builder.build();
    final doc = engine.parse(Format.markdown, 'Ça **va**, ÇA va');

    expect(doc.query(find.searchResults()), isNull);
    expect(doc.can(find.replaceAll(replacement: 'x')), isFalse);
    expect(doc.run(find.setSearchTerm(term: 'ça')), isTrue);

    var results = doc.query(find.searchResults());
    expect(results?.term, 'ça');
    expect(results?.count, 2);
    expect(results?.current, isNull);
    expect(results?.caseSensitive, isFalse);

    doc.select(const Position([0], 0));
    expect(doc.run(find.nextSearchResult()), isTrue);
    expect(doc.query(find.searchResults())?.current, 0);
    expect(doc.run(find.previousSearchResult()), isTrue);
    expect(doc.query(find.searchResults())?.current, 1, reason: 'it wraps');
    expect(doc.selection, (
      anchor: const Position([0], 7),
      head: const Position([0], 9),
    ));

    doc.run(find.setCaseSensitive(caseSensitive: true));
    results = doc.query(find.searchResults());
    expect(results?.count, 0);
    expect(results?.caseSensitive, isTrue);

    doc
      ..run(find.setCaseSensitive(caseSensitive: false))
      ..run(find.setSearchTerm(term: 'va'));
    expect(doc.run(find.replaceAll(replacement: '🐟')), isTrue);
    expect(doc.export(Format.markdown), 'Ça **🐟**, ÇA 🐟');
    expect(doc.query(bold.isBold()), isFalse);
    expect(doc.undo(), isTrue);
    expect(doc.export(Format.markdown), 'Ça **va**, ÇA va');
    expect(doc.undo(), isFalse, reason: 'replacing all is one step');
  });
}

/// The table of contents lists headings as typed records.
void listsTheOutline() {
  test('the table of contents is a typed list of records', () async {
    final builder = EngineBuilder();
    builder.add(Extensions.heading);
    final toc = builder.add(Extensions.tableOfContents);
    final engine = await builder.build();
    final doc = engine.parse(
      Format.markdown,
      '# Intro\n\n### Détail\n\n## Part',
    )..select(const Position([2], 1));

    final entries = doc.query(toc.tableOfContents());
    expect(entries.map((entry) => '${entry.number} ${entry.text}'), [
      '1 Intro',
      '1.1 Détail',
      '1.2 Part',
    ]);
    expect(entries.map((entry) => entry.level), [1, 3, 2]);
    expect(entries.map((entry) => entry.active), [false, false, true]);

    expect(doc.can(toc.goToHeading(index: 1)), isTrue);
    expect(doc.run(toc.goToHeading(index: 1)), isTrue);
    expect(doc.selection?.head, const Position([1], 0));
    expect(doc.run(toc.goToHeading(index: 3)), isFalse);
    expect(
      () => doc.run(toc.goToHeading(index: -1)),
      throwsCode(ErrorCode.invalidArgs),
    );
  });
}

/// Comments overlap, write HTML, and converge between peers.
void tracksComments() {
  test('comments overlap, answer typed records, and write HTML', () async {
    final builder = EngineBuilder();
    final comments = builder.add(Extensions.comments);
    final engine = await builder.build();
    final doc = engine.parse(Format.text, 'Shared words');

    doc.select(const Position([0], 3));
    expect(doc.can(comments.setComment(threadId: 'a')), isFalse);

    doc
      ..selectAll()
      ..run(comments.setComment(threadId: 'a'))
      ..select(const Position([0], 7), const Position([0], 12))
      ..run(comments.setComment(threadId: 'b'));
    expect(doc.run(comments.setComment(threadId: 'b')), isFalse);
    expect(doc.query(comments.activeComments()), ['a', 'b']);
    expect(doc.query(comments.comments()), [
      (threadId: 'a', text: 'Shared words'),
      (threadId: 'b', text: 'words'),
    ]);
    expect(
      doc.export(Format.html),
      '<p><span data-type="comment" data-threads="a">Shared </span>'
      '<span data-type="comment" data-threads="a b">words</span></p>',
    );
    expect(doc.export(Format.markdown), 'Shared words');

    final json = doc.toJson();
    final again = engine.fromJson(json);
    expect(again.query(comments.comments()), doc.query(comments.comments()));

    expect(doc.run(comments.unsetComment(threadId: 'a')), isTrue);
    expect(doc.query(comments.comments()), [(threadId: 'b', text: 'words')]);
  });
  test('comments made on two documents converge over Yjs', () async {
    final builder = EngineBuilder();
    final comments = builder.add(Extensions.comments);
    final engine = await builder.build();
    final first = engine.parse(Format.text, 'Shared words here')..share(1);
    final second = engine.fromYjs(first.encodeState(), clientId: 2);
    first.takeUpdates();

    first
      ..select(const Position([0], 0), const Position([0], 6))
      ..run(comments.setComment(threadId: 'a'));
    second
      ..select(const Position([0], 13), const Position([0], 17))
      ..run(comments.setComment(threadId: 'b'));
    final fromFirst = first.takeUpdates();
    second.takeUpdates().forEach(first.applyUpdate);
    fromFirst.forEach(second.applyUpdate);

    expect(first.toJson(), second.toJson());
    expect(second.query(comments.comments()), [
      (threadId: 'a', text: 'Shared'),
      (threadId: 'b', text: 'here'),
    ]);
    expect(second.validate(), (rejectedBlocks: 0, waiting: false));
  });
}

/// Block moves and clearing formatting run through their handles.
void movesAndClearsBlocks() {
  test('block moves and clearing formatting go through handles', () async {
    final builder = EngineBuilder();
    final drag = builder.add(Extensions.dragHandle);
    final clear = builder.add(Extensions.clearFormatting);
    builder
      ..add(Extensions.heading)
      ..add(Extensions.bold);
    final engine = await builder.build();
    final doc = engine.parse(Format.markdown, 'one\n\n## **two**\n\nthree')
      ..select(const Position([1], 1));

    expect(doc.can(drag.moveBlockUp()), isTrue);
    expect(doc.run(drag.moveBlockUp()), isTrue);
    expect(doc.press('Alt-ArrowDown'), isTrue);
    expect(doc.press('Alt-ArrowDown'), isTrue);
    expect(doc.export(Format.markdown), 'one\n\nthree\n\n## **two**');
    expect(doc.run(drag.moveBlocksTo(index: 0)), isTrue);
    expect(doc.run(drag.moveBlocksTo(index: 1)), isFalse);
    expect(
      () => doc.run(drag.moveBlocksTo(index: -1)),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(doc.undo(), isTrue);
    expect(doc.export(Format.markdown), 'one\n\nthree\n\n## **two**');

    doc.selectAll();
    expect(doc.run(clear.clearFormatting()), isTrue);
    expect(doc.export(Format.markdown), 'one\n\nthree\n\ntwo');
    expect(doc.can(clear.clearNodes()), isFalse);
    expect(doc.can(clear.unsetAllMarks()), isFalse);
    expect(doc.undo(), isTrue);
    doc.selectAll();
    expect(doc.press('Mod-\\'), isTrue);
    expect(doc.export(Format.markdown), 'one\n\nthree\n\ntwo');
  });
}

/// Wrong arguments and missing extensions fail with stable codes.
void reportsErrors() {
  test('document tools refuse wrong arguments with stable codes', () async {
    final builder = EngineBuilder()
      ..add(Extensions.findAndReplace)
      ..add(Extensions.comments);
    final engine = await builder.build();
    final doc = engine.parse(Format.text, 'text');

    expect(
      () => doc.runNamed('setSearchTerm'),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('setSearchTerm', {'term': 1}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('setComment', {'thread': 'a'}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('moveBlockUp'),
      throwsCode(ErrorCode.unknownName),
      reason: 'dragHandle is not registered',
    );
    expect(doc.export(Format.text), 'text');
  });
}

/// The metadata describes every document tool.
void describesTheMetadata() {
  test('every document tool has a handle and metadata', () async {
    final builder = EngineBuilder();
    for (final extension in <Extension<ExtensionHandle>>[
      Extensions.findAndReplace,
      Extensions.tableOfContents,
      Extensions.comments,
      Extensions.dragHandle,
      Extensions.clearFormatting,
    ]) {
      builder.add(extension);
    }
    final engine = await builder.build();
    final byName = {
      for (final entry in engine.metadata.cast<Map<String, Object?>>())
        entry['name']: entry,
    };
    List<Object?> names(String extension, String key) => [
      for (final item in byName[extension]![key]! as List<Object?>)
        (item! as Map<String, Object?>)['name'],
    ];

    expect(names('findAndReplace', 'commands'), [
      'setSearchTerm',
      'setCaseSensitive',
      'nextSearchResult',
      'previousSearchResult',
      'replace',
      'replaceAll',
    ]);
    expect(names('findAndReplace', 'queries'), ['searchResults']);
    expect(names('tableOfContents', 'commands'), ['goToHeading']);
    expect(names('comments', 'queries'), ['activeComments', 'comments']);
    expect(names('comments', 'nodes'), ['comment']);
    expect(names('dragHandle', 'commands'), [
      'moveBlockUp',
      'moveBlockDown',
      'moveBlocksTo',
    ]);
    expect(names('clearFormatting', 'commands'), [
      'unsetAllMarks',
      'clearNodes',
      'clearFormatting',
    ]);
  });
}
