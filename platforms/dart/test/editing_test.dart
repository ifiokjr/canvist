/// The editing helpers beyond the StarterKit through the Dart API: typed
/// handles for each, metadata that matches Rust's, stable error codes, and
/// peers that align concurrently and converge.
library;

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

typedef Editing = ({
  TextAlignHandle textAlign,
  TypographyHandle typography,
  CharacterCountHandle characterCount,
  PlaceholderHandle placeholder,
});

Future<({Engine engine, Editing kit})> editingEngine() async {
  final builder = EngineBuilder();
  Extensions.starterKit.forEach(builder.add);
  final kit = (
    textAlign: builder.add(Extensions.textAlign),
    typography: builder.add(Extensions.typography),
    characterCount: builder.add(Extensions.characterCount),
    placeholder: builder.add(Extensions.placeholder),
  );

  return (engine: await builder.build(), kit: kit);
}

Matcher throwsCode(ErrorCode code) =>
    throwsA(isA<CanvistException>().having((e) => e.code, 'code', code));

const editingNames = {
  'textAlign',
  'typography',
  'characterCount',
  'placeholder',
};

void main() {
  test('every editing extension is generated beyond the StarterKit', () {
    final kit = {for (final e in Extensions.starterKit) e.name};
    final all = {for (final e in Extensions.all) e.name};

    expect(kit.intersection(editingNames), isEmpty);
    expect(all, containsAll(editingNames));
  });

  test('the metadata names every registered editing extension', () async {
    final (:engine, kit: _) = await editingEngine();
    final names = engine.metadata
        .map((e) => (e! as Map<String, Object?>)['name'])
        .toSet();

    expect(names, containsAll(editingNames));
  });

  test('alignment sets, toggles, unsets, and undoes', () async {
    final (:engine, :kit) = await editingEngine();
    final doc = engine.parse(Format.markdown, '# Title\n\nBody')..selectAll();

    expect(doc.can(kit.textAlign.setTextAlign(alignment: 'center')), isTrue);
    expect(doc.run(kit.textAlign.setTextAlign(alignment: 'center')), isTrue);
    expect(doc.query(kit.textAlign.textAlign()), 'center');
    expect(
      doc.export(Format.html),
      '<h1 style="text-align: center">Title</h1>'
      '<p style="text-align: center">Body</p>',
    );
    expect(doc.run(kit.textAlign.toggleTextAlign(alignment: 'center')), isTrue);
    expect(doc.query(kit.textAlign.textAlign()), isNull);
    expect(doc.undo(), isTrue);
    expect(doc.query(kit.textAlign.textAlign()), 'center');
    expect(doc.run(kit.textAlign.unsetTextAlign()), isTrue);
    expect(doc.run(kit.textAlign.unsetTextAlign()), isFalse);

    expect(doc.press('Mod-Shift-r'), isTrue);
    expect(doc.query(kit.textAlign.textAlign()), 'right');

    final json = doc.toJson();
    expect(engine.fromJson(json).toJson(), json, reason: 'JSON reads back');
  });

  test('typography rewrites typed text and Backspace puts it back', () async {
    final (:engine, kit: _) = await editingEngine();
    final doc = engine.create()..typeText('"Hi" -- (c)');

    expect(doc.export(Format.text), '“Hi” — ©');
    expect(doc.press('Backspace'), isTrue);
    expect(doc.export(Format.text), '“Hi” — (c)');
    doc.insertText(' ->');
    expect(doc.export(Format.text), '“Hi” — (c) ->', reason: 'no rules');
  });

  test('character counts and limits', () async {
    final (:engine, :kit) = await editingEngine();
    final doc = engine.parse(Format.text, '👍🏽 one')
      ..select(const Position([0], 8));

    expect(doc.query(kit.characterCount.characters()), 5);
    expect(doc.query(kit.characterCount.words()), 2);
    expect(doc.query(kit.characterCount.characterLimit()), isNull);

    doc
      ..run(kit.characterCount.setCharacterLimit(limit: 6))
      ..typeText(' two');
    expect(doc.export(Format.text), '👍🏽 one ');
    expect(doc.query(kit.characterCount.characterLimit()), 6);

    doc
      ..run(kit.characterCount.setCharacterLimit(limit: null))
      ..typeText('two');
    expect(doc.query(kit.characterCount.characters()), 9);
  });

  test('placeholders follow the caret', () async {
    final (:engine, :kit) = await editingEngine();
    final doc = engine.create();

    expect(doc.query(kit.placeholder.placeholder()), 'Write something …');
    doc.run(kit.placeholder.setPlaceholder(placeholder: 'Say hi'));
    expect(doc.query(kit.placeholder.placeholder()), 'Say hi');
    doc.typeText('hi');
    expect(doc.query(kit.placeholder.placeholder()), isNull);
  });

  test('editing errors carry stable codes', () async {
    final (:engine, :kit) = await editingEngine();
    final doc = engine.parse(Format.text, 'x');

    expect(
      () => doc.run(kit.textAlign.setTextAlign(alignment: 'middle')),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.run(kit.characterCount.setCharacterLimit(limit: -1)),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.run(kit.characterCount.setCharacterLimit(limit: 1 << 32)),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('setPlaceholder', {'placeholder': 3}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(doc.export(Format.html), '<p>x</p>');

    final (engine: plain, kit: _) = await Engine.starterKit();
    expect(
      () => plain.create().runNamed('setTextAlign', {'alignment': 'left'}),
      throwsCode(ErrorCode.unknownName),
    );
  });

  test('two peers aligning different blocks converge', () async {
    final (:engine, :kit) = await editingEngine();
    final first = engine.parse(Format.markdown, '# Title\n\nBody')..share(1);
    final second = engine.fromYjs(first.encodeState(), clientId: 2);

    first
      ..select(const Position([0], 0))
      ..run(kit.textAlign.setTextAlign(alignment: 'center'));
    second
      ..select(const Position([1], 0))
      ..run(kit.textAlign.setTextAlign(alignment: 'justify'));

    final fromFirst = first.takeUpdates();
    final fromSecond = second.takeUpdates();
    fromFirst.forEach(second.applyUpdate);
    fromSecond.forEach(first.applyUpdate);

    expect(first.toJson(), second.toJson());
    expect(first.validate(), (rejectedBlocks: 0, waiting: false));
    expect(
      first.export(Format.html),
      '<h1 style="text-align: center">Title</h1>'
      '<p style="text-align: justify">Body</p>',
    );
  });
}
