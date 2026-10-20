/// The interactive view UI hosts build on, measured by [FixedMeasurer] so
/// every position is predictable: 16 px text, 9.6 px per character.
library;

import 'dart:typed_data';

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

const _width = 400.0;
const _height = 600.0;

/// Body text advances 9.6 px per character at the default 16 px.
const _advance = 16 * 0.6;

final _painted = Expando<ViewDisplay>();

void main() {
  late Engine engine;
  late StarterKitHandles kit;

  setUpAll(() async {
    (:engine, :kit) = await Engine.starterKit();
  });

  EditorView viewOf(Doc doc, {TextMeasurer measurer = const FixedMeasurer()}) {
    final view = doc.view(measurer)..setWidth(_width);
    addTearDown(view.dispose);
    return view;
  }

  /// What the view paints now: a frame's display, or the last one when
  /// the frame says nothing changed.
  List<ViewItem> items(EditorView view, {ViewItemKind? kind}) {
    final display = view.frame(0, _height).display ?? _painted[view]!;
    _painted[view] = display;
    return [
      for (final item in display.items)
        if (kind == null || item.kind == kind) item,
    ];
  }

  String text(EditorView view) =>
      [for (final item in items(view, kind: ViewItemKind.text)) item.text]
          .join();

  test('a frame paints the laid-out text and skips unchanged frames', () {
    final view = viewOf(engine.parse(Format.markdown, '# Title\n\nBody text'));
    final first = view.frame(0, _height);
    _painted[view] = first.display;
    final runs = [
      for (final item in first.display!.items)
        if (item.kind == ViewItemKind.text) item,
    ];

    expect(runs.map((run) => run.text).join(), 'TitleBody text');
    expect(first.height, greaterThan(0));
    expect(first.laidOut, 2);
    expect(view.fonts, hasLength(2), reason: 'a heading font and a body font');
    expect(view.fonts[runs.first.font].weight, 700);
    expect(view.frame(0, _height).display, isNull, reason: 'nothing changed');
  });

  test('keys and typing go through the editor, with input rules', () {
    final doc = engine.create();
    final view = viewOf(doc);

    for (final character in '## Hi'.split('')) {
      view.insertText(character);
    }

    expect(doc.export(Format.markdown), '## Hi');
    expect(view.keyDown('Enter'), isTrue);
    view.insertText('next');
    expect(view.keyDown('ArrowLeft', shift: true), isTrue);
    expect(view.keyDown('b', ctrl: true), isTrue, reason: 'Mod is Ctrl');
    expect(doc.export(Format.markdown), '## Hi\n\nnex**t**');
    expect(view.keyDown('ArrowUp'), isTrue);
    expect(doc.selection?.head.path, [0]);
    expect(view.keyDown('x'), isFalse, reason: 'unbound keys are text');
  });

  test('pointer presses place the caret and drags select', () {
    final doc = engine.parse(Format.markdown, 'hello wide world');
    final view = viewOf(doc);
    final baseline = items(view, kind: ViewItemKind.text).first;
    final y = baseline.y + baseline.height / 2;

    expect(view.pointerDown(_advance * 6, y), isTrue);
    expect(doc.selection, (
      anchor: const Position([0], 6),
      head: const Position([0], 6),
    ));
    view
      ..pointerMove(_advance * 10, y)
      ..pointerUp();
    expect(view.copySelection()?.text, 'wide');
    expect(view.isInSelection(_advance * 8, y), isTrue);

    view.pointerDown(_advance * 13, y, clicks: 2);
    expect(view.copySelection()?.text, 'world');
    view.pointerUp();

    expect(
      view.moveSelectionEnd(_advance * 6, y, start: true),
      isTrue,
      reason: 'a handle drags one end',
    );
    expect(view.copySelection()?.text, 'wide world');
    expect(items(view, kind: ViewItemKind.selection), isNotEmpty);
  });

  test('input methods edit the head block in UTF-16 offsets', () {
    final doc = engine.parse(Format.markdown, 'a😀 b');
    final view = viewOf(doc);
    doc.select(const Position([0], 5));

    expect(
      view.editState,
      const ViewEditState(text: 'a😀 b', anchor: 5, head: 5),
    );
    expect(view.replaceText(4, 5, 'B!'), isTrue);
    expect(doc.export(Format.text), 'a😀 B!');

    expect(view.setComposition('か'), isTrue);
    expect(view.editState?.composingStart, 6);
    expect(view.editState?.composingEnd, 7);
    expect(items(view, kind: ViewItemKind.composition), isNotEmpty);
    expect(view.compositionBounds, isNotNull);
    view.setComposition('かな');
    expect(view.commitComposition('仮名'), isTrue);
    expect(doc.export(Format.text), 'a😀 B!仮名');
    expect(view.editState?.composingStart, isNull);

    view.setComposition('x');
    view.cancelComposition();
    expect(doc.export(Format.text), 'a😀 B!仮名');
    expect(view.selectInBlock(0, 1), isTrue);
    expect(view.copySelection()?.text, 'a');
  });

  test('commands from handles, undo, and redo go through the view', () {
    final doc = engine.parse(Format.markdown, 'Make me bold');
    final view = viewOf(doc);
    doc.selectAll();

    expect(view.run(kit.bold.toggleBold()), isTrue);
    expect(doc.query(kit.bold.isBold()), isTrue);
    final runs = items(view, kind: ViewItemKind.text);
    expect(runs.every((run) => view.fonts[run.font].weight == 700), isTrue);
    expect(view.undo(), isTrue);
    expect(doc.export(Format.markdown), 'Make me bold');
    expect(view.redo(), isTrue);
    expect(doc.export(Format.markdown), '**Make me bold**');

    final other = EngineBuilder();
    final bold = other.add(Extensions.bold);
    expect(() => view.run(bold.toggleBold()), throwsArgumentError);
  });

  test('changes made on the document reach the layout', () {
    final doc = engine.parse(Format.markdown, 'One');
    final view = viewOf(doc);
    expect(text(view), 'One');

    doc
      ..select(const Position([0], 3))
      ..insertText(' two');
    expect(text(view), 'One two');
  });

  test("collaborators' updates reach the view", () {
    final alice = engine.parse(Format.markdown, 'Shared');
    alice.share(1);
    final bob = engine.fromYjs(alice.encodeState(), clientId: 2);
    final view = viewOf(bob);
    expect(text(view), 'Shared');

    alice
      ..select(const Position([0], 6))
      ..insertText(' note')
      ..press('Enter')
      ..insertText('more');

    for (final update in alice.takeUpdates()) {
      bob.applyUpdate(update);
    }

    expect(text(view), 'Shared notemore');
    expect(view.semantics.text, 'Shared note\nmore');

    bob.select(const Position([1], 4));
    view.insertText('!');
    for (final update in bob.takeUpdates()) {
      alice.applyUpdate(update);
    }
    expect(alice.export(Format.text), 'Shared note\nmore!');
  });

  test('semantics expose the plain text and select through it', () {
    final doc = engine.parse(Format.markdown, '# Hi\n\n- one\n- two');
    final view = viewOf(doc);

    expect(view.semantics.text, 'Hi\none\ntwo');
    expect(view.selectPlain(3, 6), isTrue);
    expect(view.copySelection()?.text, 'one');
    expect(view.semantics.anchor, 3);
    expect(view.semantics.head, 6);
  });

  test('the caret shows while focused and moves with the selection', () {
    final doc = engine.parse(Format.markdown, 'abc');
    final view = viewOf(doc);
    doc.select(const Position([0], 2));

    expect(items(view, kind: ViewItemKind.caret), isEmpty);
    view.setFocused(true);
    final caret = items(view, kind: ViewItemKind.caret).single;
    expect(caret.x, closeTo(_advance * 2, 1));
    expect(view.caretRect?.x, closeTo(_advance * 2, 1));
    view.setCaretOn(false);
    expect(items(view, kind: ViewItemKind.caret), isEmpty);
    expect(view.selectionEnds?.collapsed, isTrue);
  });

  test('clipboard formats round trip through paste', () {
    final doc = engine.parse(Format.markdown, '**bold** plain');
    final view = viewOf(doc);
    doc.selectAll();

    final copied = view.cutSelection()!;
    expect(copied.text, 'bold plain');
    expect(copied.html, contains('<strong>bold</strong>'));
    expect(doc.export(Format.text), '');
    expect(view.pasteHtml(copied.html), isTrue);
    expect(doc.export(Format.markdown), '**bold** plain');
    expect(view.pasteText(' more'), isTrue);
    expect(doc.export(Format.text), 'bold plain more');
  });

  test('server rendering matches the view laid out with the same font', () {
    final doc = engine.parse(Format.markdown, '# Server\n\nRendered *text*');
    final (:html, :css) = doc.toHtml(_width);

    expect(html, contains('<h1'));
    expect(css, contains('.cv-doc'));
    expect(doc.toSvg(_width), startsWith('<svg'));

    final server = doc.serverDisplay(_width);
    final runs = [
      for (final item in server.items)
        if (item.kind == ViewItemKind.text) item.text,
    ];
    expect(runs.join(), 'ServerRendered text');
    expect(server.fonts, isNotEmpty);
  });

  test('a measurer that throws fails the call that needed it', () {
    final view = viewOf(engine.create(), measurer: const _BrokenMeasurer());

    expect(() => view.frame(0, _height), throwsA(isA<StateError>()));
    expect(view.frame(0, _height).height, greaterThan(0));
  });

  test('typing fast never fails and keeps every character', () {
    final doc = engine.create();
    final view = viewOf(doc)..setFocused(true);
    final typed = StringBuffer();

    for (var index = 0; index < 2000; index++) {
      final character = String.fromCharCode(0x61 + index % 26);
      typed.write(character);
      view.insertText(character);

      if (index % 97 == 96) {
        view.keyDown('Enter');
        typed.write('\n');
      }

      if (index % 5 == 0) {
        view.frame(0, _height);
      }
    }

    expect(doc.export(Format.text), typed.toString());
  });
}

final class _BrokenMeasurer implements TextMeasurer {
  const _BrokenMeasurer();

  static var _calls = 0;

  @override
  FontMetrics metrics(FontSpec font) {
    if (_calls++ == 0) {
      throw StateError('the font is not loaded');
    }

    return const FixedMeasurer().metrics(font);
  }

  @override
  void measure(FontSpec font, String text, Float32List advances) =>
      const FixedMeasurer().measure(font, text, advances);
}
