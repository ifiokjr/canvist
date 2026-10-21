/// The formatting extensions beyond the StarterKit through the Dart API:
/// typed handles for every one, metadata that matches Rust's, stable error
/// codes, and peers that format concurrently and converge.
library;

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

/// Handles for the StarterKit plus every formatting extension.
typedef Formatting = ({
  SubscriptHandle subscript,
  SuperscriptHandle superscript,
  HighlightHandle highlight,
  TextStyleHandle textStyle,
  ColorHandle color,
  BackgroundColorHandle backgroundColor,
  FontFamilyHandle fontFamily,
  FontSizeHandle fontSize,
  LineHeightHandle lineHeight,
});

Future<({Engine engine, Formatting kit})> formattingEngine() async {
  final builder = EngineBuilder();
  Extensions.starterKit.forEach(builder.add);
  final kit = (
    subscript: builder.add(Extensions.subscript),
    superscript: builder.add(Extensions.superscript),
    highlight: builder.add(Extensions.highlight),
    textStyle: builder.add(Extensions.textStyle),
    color: builder.add(Extensions.color),
    backgroundColor: builder.add(Extensions.backgroundColor),
    fontFamily: builder.add(Extensions.fontFamily),
    fontSize: builder.add(Extensions.fontSize),
    lineHeight: builder.add(Extensions.lineHeight),
  );

  return (engine: await builder.build(), kit: kit);
}

Matcher throwsCode(ErrorCode code) =>
    throwsA(isA<CanvistException>().having((e) => e.code, 'code', code));

const formattingNames = {
  'subscript',
  'superscript',
  'highlight',
  'textStyle',
  'color',
  'backgroundColor',
  'fontFamily',
  'fontSize',
  'lineHeight',
};

void main() {
  test('every formatting extension is generated beyond the StarterKit', () {
    final kit = {for (final e in Extensions.starterKit) e.name};
    final all = {for (final e in Extensions.all) e.name};

    expect(kit.intersection(formattingNames), isEmpty);
    expect(all, containsAll(formattingNames));
  });

  test('the metadata names every registered formatting extension', () async {
    final (:engine, kit: _) = await formattingEngine();
    final names = engine.metadata
        .map((e) => (e! as Map<String, Object?>)['name'])
        .toSet();

    expect(names, containsAll(formattingNames));
  });

  test('scripts toggle, exclude each other, and undo', () async {
    final (:engine, :kit) = await formattingEngine();
    final doc = engine.parse(Format.text, 'x2')
      ..select(const Position([0], 1), const Position([0], 2));

    expect(doc.can(kit.subscript.toggleSubscript()), isTrue);
    expect(doc.run(kit.subscript.toggleSubscript()), isTrue);
    expect(doc.query(kit.subscript.isSubscript()), isTrue);
    expect(doc.run(kit.superscript.setSuperscript()), isTrue);
    expect(doc.query(kit.subscript.isSubscript()), isFalse);
    expect(doc.export(Format.markdown), 'x<sup>2</sup>');

    expect(doc.run(kit.superscript.unsetSuperscript()), isTrue);
    expect(doc.export(Format.html), '<p>x2</p>');
    expect(doc.undo(), isTrue);
    expect(doc.query(kit.superscript.isSuperscript()), isTrue);
    expect(doc.press('Mod-.'), isTrue);
    expect(doc.query(kit.superscript.isSuperscript()), isFalse);
  });

  test(
    'highlights take a colour, the theme colour, or the typed rule',
    () async {
      final (:engine, :kit) = await formattingEngine();
      final doc = engine.create()..typeText('a ==glow==');

      expect(doc.export(Format.html), '<p>a <mark>glow</mark></p>');
      expect(doc.press('Backspace'), isTrue);
      expect(
        doc.export(Format.text),
        'a ==glow==',
        reason: 'Backspace undoes the rule',
      );

      doc
        ..selectAll()
        ..run(kit.highlight.setHighlight(color: '#fef08a'));
      expect(doc.query(kit.highlight.highlightColor()), '#fef08a');
      expect(doc.run(kit.highlight.toggleHighlight(color: '#fef08a')), isTrue);
      expect(doc.query(kit.highlight.isHighlight()), isFalse);

      doc.run(kit.highlight.setHighlight());
      expect(doc.query(kit.highlight.highlightColor()), isNull);
      expect(doc.run(kit.highlight.unsetHighlight()), isTrue);
      expect(doc.query(kit.highlight.isHighlight()), isFalse);
    },
  );

  test('every style attribute sets, reads back, and clears', () async {
    final (:engine, :kit) = await formattingEngine();
    final doc = engine.parse(Format.text, 'ink')
      ..selectAll()
      ..run(kit.color.setColor(color: '#958df1'))
      ..run(kit.backgroundColor.setBackgroundColor(backgroundColor: '#fafafa'))
      ..run(kit.fontFamily.setFontFamily(fontFamily: 'Georgia'))
      ..run(kit.fontSize.setFontSize(fontSize: '18px'))
      ..run(kit.lineHeight.setLineHeight(lineHeight: '1.5'));

    expect(doc.query(kit.color.color()), '#958df1');
    expect(doc.query(kit.backgroundColor.backgroundColor()), '#fafafa');
    expect(doc.query(kit.fontFamily.fontFamily()), 'Georgia');
    expect(doc.query(kit.fontSize.fontSize()), '18px');
    expect(doc.query(kit.lineHeight.lineHeight()), '1.5');
    expect(
      doc.export(Format.html),
      '<p><span style="color: #958df1; background-color: #fafafa; '
      'font-family: Georgia; font-size: 18px; line-height: 1.5">ink</span></p>',
    );

    final json = doc.toJson();
    expect(engine.fromJson(json).toJson(), json, reason: 'JSON reads back');

    doc
      ..run(kit.color.unsetColor())
      ..run(kit.backgroundColor.unsetBackgroundColor())
      ..run(kit.fontFamily.unsetFontFamily())
      ..run(kit.fontSize.unsetFontSize());
    expect(doc.query(kit.fontSize.fontSize()), isNull);
    expect(doc.query(kit.lineHeight.lineHeight()), '1.5');
    doc.run(kit.lineHeight.unsetLineHeight());
    expect(doc.export(Format.html), '<p>ink</p>');
    expect(doc.run(kit.textStyle.removeEmptyTextStyle()), isTrue);
  });

  test('formatting errors carry stable codes', () async {
    final (:engine, kit: _) = await formattingEngine();
    final doc = engine.parse(Format.text, 'x')..selectAll();

    expect(
      () => doc.runNamed('setFontSize', {'fontSize': 18}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('setHighlight', {'color': true}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(() => doc.runNamed('setColor'), throwsCode(ErrorCode.invalidArgs));
    expect(
      () => doc.runNamed('toggleSpoiler'),
      throwsCode(ErrorCode.unknownName),
    );
    expect(doc.export(Format.html), '<p>x</p>');

    final bare = EngineBuilder();
    final color = bare.add(Extensions.color);
    final withoutStyle = (await bare.build()).parse(Format.text, 'x')
      ..selectAll();
    expect(
      () => withoutStyle.run(color.setColor(color: 'red')),
      throwsCode(ErrorCode.editor),
    );

    final (engine: plain, kit: _) = await Engine.starterKit();
    expect(
      () => plain.fromJson({
        'type': 'doc',
        'content': [
          {
            'type': 'paragraph',
            'content': [
              {
                'type': 'text',
                'text': 'x',
                'marks': [
                  {'type': 'highlight'},
                ],
              },
            ],
          },
        ],
      }),
      throwsCode(ErrorCode.schema),
    );
  });

  test('two peers formatting concurrently converge', () async {
    final (:engine, :kit) = await formattingEngine();
    final first = engine.parse(Format.text, 'Hello world')..share(1);
    final second = engine.fromYjs(first.encodeState(), clientId: 2);

    first
      ..select(const Position([0], 0), const Position([0], 5))
      ..run(kit.color.setColor(color: '#e8590c'));
    second
      ..select(const Position([0], 3), const Position([0], 11))
      ..run(kit.highlight.setHighlight(color: '#fef08a'))
      ..run(kit.subscript.setSubscript());

    final fromFirst = first.takeUpdates();
    final fromSecond = second.takeUpdates();
    fromFirst.forEach(second.applyUpdate);
    fromSecond.forEach(first.applyUpdate);

    expect(first.toJson(), second.toJson());
    expect(first.validate(), (rejectedBlocks: 0, waiting: false));
    expect(
      first.export(Format.markdown),
      'Hel<sub>==lo world==</sub>',
      reason: 'Markdown keeps the highlight and script, not the colour',
    );
  });
}
