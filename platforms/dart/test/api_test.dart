/// The Dart API beyond the conformance fixtures: typed handles, engines
/// with some extensions, errors, and collaboration between documents.
library;

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

void main() {
  test('handles run typed commands and answer typed queries', () async {
    final (:engine, :kit) = await Engine.starterKit();
    final doc = engine.parse(Format.markdown, 'Title')
      ..select(const Position([0], 0), const Position([0], 5));

    expect(doc.query(kit.heading.headingLevel()), isNull);
    expect(doc.can(kit.heading.setHeading(level: 2)), isTrue);
    expect(doc.run(kit.heading.setHeading(level: 2)), isTrue);
    expect(doc.query(kit.heading.headingLevel()), 2);

    doc.run(kit.bold.toggleBold());
    expect(doc.query(kit.bold.isBold()), isTrue);
    expect(doc.export(Format.markdown), '## **Title**');

    doc.run(kit.codeBlock.setCodeBlock(language: 'dart'));
    expect(doc.query(kit.codeBlock.codeBlockLanguage()), 'dart');
  });

  test('an engine has only the extensions registered on it', () async {
    final builder = EngineBuilder();
    final bold = builder.add(Extensions.bold);
    final engine = await builder.build();
    final doc = engine.parse(Format.markdown, '# Not a heading')..selectAll();

    expect(doc.export(Format.html), '<p>Not a heading</p>');
    expect(doc.run(bold.toggleBold()), isTrue);
    expect(doc.query(builder.paragraph.isParagraph()), isTrue);
    expect(
      () => doc.runNamed('toggleItalic'),
      throwsA(
        isA<CanvistException>().having(
          (e) => e.code,
          'code',
          ErrorCode.unknownName,
        ),
      ),
    );

    final names = engine.metadata
        .map((e) => (e! as Map<String, Object?>)['name'])
        .toSet();
    expect(names, {'doc', 'paragraph', 'baseKeymap', 'undoInputRule', 'bold'});
  });

  test('extensions beyond the StarterKit have typed handles', () async {
    final builder = EngineBuilder();
    final sub = builder.add(Extensions.subscript);
    final highlight = builder.add(Extensions.highlight);
    builder.add(Extensions.textStyle);
    final color = builder.add(Extensions.color);
    final engine = await builder.build();
    final doc = engine.parse(Format.text, 'H2O')
      ..select(const Position([0], 1), const Position([0], 2));

    expect(doc.run(sub.toggleSubscript()), isTrue);
    expect(doc.query(sub.isSubscript()), isTrue);

    doc
      ..selectAll()
      ..run(highlight.setHighlight(color: '#fef08a'))
      ..run(color.setColor(color: '#e8590c'));
    expect(doc.query(highlight.highlightColor()), '#fef08a');
    expect(doc.query(color.color()), '#e8590c');

    doc.run(color.unsetColor());
    expect(doc.query(color.color()), isNull);
    expect(doc.export(Format.markdown), '==H<sub>2</sub>O==');
  });

  test('queries answer typed records and lists', () async {
    final builder = EngineBuilder();
    final mention = builder.add(Extensions.mention);
    final emoji = builder.add(Extensions.emoji);
    final engine = await builder.build();
    final doc = engine.create()..typeText('Hi @ad');

    final suggestion = doc.query(mention.mentionSuggestion());
    expect(suggestion?.char, '@');
    expect(suggestion?.query, 'ad');

    final found = doc.query(emoji.searchEmoji(query: 'tada', limit: 1));
    expect(found.single.emoji, '🎉');
    expect(found.single.name, 'tada');

    doc.run(mention.insertMention(id: '7', label: 'Ada'));
    expect(doc.query(mention.mentionSuggestion()), isNull);
    expect(doc.export(Format.text), 'Hi @Ada ');
  });

  test('a handle from one engine is refused by another', () async {
    final (:engine, kit: _) = await Engine.starterKit();
    final other = EngineBuilder();
    final bold = other.add(Extensions.bold);
    await other.build();

    expect(() => engine.create().run(bold.toggleBold()), throwsArgumentError);
  });

  test('every StarterKit extension in the metadata has a handle', () async {
    final (:engine, kit: _) = await Engine.starterKit();
    final names = engine.metadata
        .map((e) => (e! as Map<String, Object?>)['name'])
        .toSet();
    final registered = {
      for (final extension in Extensions.starterKit) extension.name,
    };

    expect(
      names.difference({'doc', 'paragraph', 'baseKeymap', 'undoInputRule'}),
      registered,
    );
  });

  test('arguments are checked by Rust and fail with stable codes', () async {
    final (:engine, :kit) = await Engine.starterKit();
    final doc = engine.create();

    expect(
      () => doc.run(kit.heading.setHeading(level: 9)),
      throwsA(
        isA<CanvistException>().having(
          (e) => e.code,
          'code',
          ErrorCode.invalidArgs,
        ),
      ),
    );
    expect(
      () => engine.parse(Format.json, '{ nope'),
      throwsA(
        isA<CanvistException>().having((e) => e.code, 'code', ErrorCode.parse),
      ),
    );
    expect(
      () => doc.select(const Position([0], 3)),
      throwsA(
        isA<CanvistException>().having(
          (e) => e.code,
          'code',
          ErrorCode.invalidPosition,
        ),
      ),
    );
  });

  test('documents collaborate through Yjs updates', () async {
    final (:engine, :kit) = await Engine.starterKit();
    final server = engine.parse(Format.markdown, 'Hello')..share(1);
    final peer = engine.fromYjs(server.encodeState(), clientId: 2);

    peer
      ..select(const Position([0], 5))
      ..insertText(' world');
    for (final update in peer.takeUpdates()) {
      expect(server.applyUpdate(update), isTrue);
    }

    server
      ..select(const Position([0], 0), const Position([0], 5))
      ..run(kit.italic.toggleItalic());
    server.takeUpdates().forEach(peer.applyUpdate);

    expect(peer.export(Format.markdown), '*Hello* world');
    expect(server.validate(), (rejectedBlocks: 0, waiting: false));
  });

  test('snapshots keep content and selection, with UTF-16 offsets', () async {
    final (:engine, kit: _) = await Engine.starterKit();
    final doc = engine.parse(Format.text, 'a😀b')
      ..select(const Position([0], 3), const Position([0], 4))
      ..insertText('c');

    final restored = engine.restore(doc.snapshot());

    expect(restored.export(Format.text), 'a😀c');
    expect(restored.selection, (
      anchor: const Position([0], 4),
      head: const Position([0], 4),
    ));
  });
}
