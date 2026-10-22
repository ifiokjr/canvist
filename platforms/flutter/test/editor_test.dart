/// The editor widget on the host: text input with and without deltas, IME
/// composition, keys, mouse and touch selection, the toolbar, scrolling,
/// semantics, collaboration, and fast typing.
library;

import 'dart:io';
import 'dart:math';

import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:canvist_flutter/src/render.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// Pump an editor for [content] with focus, and return its controller.
Future<CanvistController> _focused(
  WidgetTester tester,
  String content, {
  Size size = const Size(400, 600),
  Format format = Format.markdown,
}) async {
  final controller = controllerFor(content, format: format);
  await pumpEditor(
    tester,
    CanvistEditor(controller: controller, autofocus: true),
    size: size,
  );
  await tester.pump();
  return controller;
}

/// The editor's padding around the document.
const _padding = 16.0;

/// The test font draws every character one em wide.
const _em = 16.0;

/// Paragraphs in the long document.
const _paragraphs = 200;

/// How many keys fast typing presses, one in [_keyEvery] a named key.
const _keystrokes = 600;
const _keyEvery = 11;
const _alphabet = 'abcdefghijklmnopqrstuvwxyz';

/// One 60 Hz frame.
const _frameMicroseconds = 16000;

void main() {
  setUpAll(loadStarterKit);

  testWidgets(
    'the editor paints the document and follows the theme',
    _paintsAndFollowsTheTheme,
  );

  testWidgets(
    'typing through the platform edits the document',
    _typesThroughThePlatform,
  );

  testWidgets(
    'platforms without deltas edit through whole values',
    _editsThroughWholeValues,
  );

  testWidgets(
    'backspace at the start of a block joins it to the one before',
    _joinsBlocksOnBackspace,
  );

  testWidgets(
    'IME composition shows preedit and commits it',
    _composesWithAnInputMethod,
  );

  testWidgets('autocorrect replaces a word behind the caret', _autocorrects);

  testWidgets(
    'typing on an Android keyboard never restarts its input',
    _typesWithoutRestartingInput,
  );
  testWidgets(
    'keys run bindings: formatting, history, and navigation',
    _runsKeyBindings,
  );

  testWidgets(
    'copy, cut, and paste keep formatting within the editor',
    _copiesCutsAndPastes,
  );

  testWidgets(
    'a mouse places the caret, drags, and double-clicks words',
    _selectsWithAMouse,
  );

  testWidgets(
    'touch selects words with handles and a toolbar',
    _selectsWithTouch,
  );

  testWidgets(
    'a long document scrolls and keeps the caret in view',
    _scrollsToTheCaret,
  );

  testWidgets(
    'semantics expose the text, selection, and actions',
    _exposesSemantics,
  );

  testWidgets(
    'assistive technology sets text with the smallest edit',
    _setsTextAsAssistiveTechnologyDoes,
  );

  testWidgets(
    "collaborators' edits appear in each other's editors",
    _collaborates,
  );

  testWidgets(
    'typing fast never fails and keeps up with every keystroke',
    _keepsUpWithFastTyping,
  );
}

Future<void> _paintsAndFollowsTheTheme(WidgetTester tester) async {
  final controller = controllerFor('# Title\n\nBody');
  await pumpEditor(tester, CanvistEditor(controller: controller));

  final editor = tester.renderObject<RenderBox>(find.byType(CanvistEditor));
  expect(editor.size, const Size(400, 600));
  expect(controller.view.fonts, isNotEmpty, reason: 'layout measured text');

  RenderEditorViewport viewport() =>
      tester.renderObject(find.byType(EditorViewport));
  final light = viewport().display!.background;
  await pumpEditor(
    tester,
    CanvistEditor(controller: controller),
    theme: ThemeData.dark(),
  );
  // The app animates from one theme to the other.
  await tester.pumpAndSettle();
  expect(viewport().display!.background, isNot(light));
}

Future<void> _typesThroughThePlatform(WidgetTester tester) async {
  final controller = await _focused(tester, '');

  expect(tester.testTextInput.isVisible, isTrue);
  expect(tester.testTextInput.editingState?['text'], EditorInput.prefix);

  await sendDeltas(tester, [typed(controller, 'Hello')]);
  expect(controller.export(Format.text), 'Hello');

  await sendDeltas(tester, [typed(controller, '\nworld')]);
  expect(controller.export(Format.text), 'Hello\nworld');
  expect(
    tester.testTextInput.editingState?['text'],
    '${EditorInput.prefix}world',
    reason: 'the platform edits the block holding the caret',
  );
}

Future<void> _editsThroughWholeValues(WidgetTester tester) async {
  final controller = await _focused(tester, '');

  var text = EditorInput.prefix;

  for (final character in '## Plan'.split('')) {
    text += character;
    tester.testTextInput.updateEditingValue(
      TextEditingValue(
        text: text,
        selection: TextSelection.collapsed(offset: text.length),
      ),
    );
    await tester.pump();
    text = EditorInput(controller.view).value.text;
  }

  expect(
    controller.export(Format.markdown),
    '## Plan',
    reason: 'input rules apply',
  );
  expect(
    tester.testTextInput.log.where(
      (call) =>
          call.method == 'TextInput.setEditingState' &&
          (call.arguments as Map<Object?, Object?>)['text'] == '\n',
    ),
    isNotEmpty,
    reason: 'the platform heard that the rule removed "## "',
  );
}

Future<void> _joinsBlocksOnBackspace(WidgetTester tester) async {
  final controller = await _focused(tester, 'one\n\ntwo');
  controller.select(const Position([1], 0));
  await tester.pump();

  final value = tester.testTextInput.editingState!;
  expect(value['text'], '\ntwo');
  await sendDeltas(tester, [
    TextEditingDeltaDeletion(
      oldText: '\ntwo',
      deletedRange: const TextRange(start: 0, end: 1),
      selection: const TextSelection.collapsed(offset: 0),
      composing: TextRange.empty,
    ),
  ]);
  expect(controller.export(Format.text), 'onetwo');
}

Future<void> _composesWithAnInputMethod(WidgetTester tester) async {
  final controller = await _focused(tester, 'Say ', format: Format.text);
  controller.select(const Position([0], 4));
  await tester.pump();

  await sendDeltas(tester, [
    typed(controller, 'に', composing: const TextRange(start: 5, end: 6)),
  ]);
  expect(controller.view.editState?.composingStart, 4);
  expect(controller.export(Format.text), 'Say に');

  await sendDeltas(tester, [
    TextEditingDeltaReplacement(
      oldText: '\nSay に',
      replacementText: 'にほ',
      replacedRange: const TextRange(start: 5, end: 6),
      selection: const TextSelection.collapsed(offset: 7),
      composing: const TextRange(start: 5, end: 7),
    ),
  ]);
  expect(controller.export(Format.text), 'Say にほ');

  // Keys belong to the input method while it composes.
  await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
  expect(controller.export(Format.text), 'Say にほ');

  await sendDeltas(tester, [
    TextEditingDeltaReplacement(
      oldText: '\nSay にほ',
      replacementText: '日本',
      replacedRange: const TextRange(start: 5, end: 7),
      selection: const TextSelection.collapsed(offset: 7),
      composing: TextRange.empty,
    ),
  ]);
  expect(controller.export(Format.text), 'Say 日本');
  expect(controller.view.editState?.composingStart, isNull);

  expect(controller.undo(), isTrue);
  expect(
    controller.export(Format.text),
    'Say ',
    reason: 'the commit is one undo step',
  );
}

Future<void> _autocorrects(WidgetTester tester) async {
  final controller = await _focused(tester, 'teh ', format: Format.text);
  controller.select(const Position([0], 4));
  await tester.pump();

  await sendDeltas(tester, [
    TextEditingDeltaReplacement(
      oldText: '\nteh ',
      replacementText: 'the',
      replacedRange: const TextRange(start: 1, end: 4),
      selection: const TextSelection.collapsed(offset: 5),
      composing: TextRange.empty,
    ),
  ]);
  expect(controller.export(Format.text), 'the ');
  expect(controller.selection?.head, const Position([0], 4));
}

Future<void> _typesWithoutRestartingInput(WidgetTester tester) async {
  final controller = await _focused(tester, 'Say', format: Format.text);
  controller.select(const Position([0], 3));
  await tester.pump();
  final keyboard = _AndroidKeyboard(tester);

  await keyboard.start();
  for (final character in ' hello world'.split('')) {
    await keyboard.type(character);
  }

  expect(controller.export(Format.text), 'Say hello world');
  expect(
    keyboard.restarts,
    0,
    reason: 'the editor leaves the words the keyboard marks alone',
  );
}

/// A keyboard that behaves as Android's do. Each state the editor sends
/// restarts its input, and every start marks the word before the caret for
/// suggestions. Letters compose over the marked word, and a space commits it.
class _AndroidKeyboard {
  _AndroidKeyboard(this.tester);

  final WidgetTester tester;

  /// How many times the editor restarted the keyboard's input.
  var restarts = 0;

  /// Restarts after which the keyboard gives up, so a loop ends the test.
  static const _giveUpAfter = 50;

  var _text = '';
  var _caret = 0;
  var _composing = TextRange.empty;
  var _heard = 0;

  Future<void> start() async {
    final state = tester.testTextInput.editingState!;
    _heard = tester.testTextInput.log.length;
    await _restart(state['text']! as String, state['selectionExtent']! as int);
  }

  Future<void> type(String character) async {
    if (character == ' ') {
      if (!_composing.isCollapsed) {
        _composing = TextRange.empty;
        await _send(
          TextEditingDeltaNonTextUpdate(
            oldText: _text,
            selection: TextSelection.collapsed(offset: _caret),
            composing: _composing,
          ),
        );
      }

      final before = _text;
      _text = '${before.substring(0, _caret)} ${before.substring(_caret)}';
      _caret++;
      return _send(
        TextEditingDeltaInsertion(
          oldText: before,
          textInserted: ' ',
          insertionOffset: _caret - 1,
          selection: TextSelection.collapsed(offset: _caret),
          composing: TextRange.empty,
        ),
      );
    }

    final word = _composing.isCollapsed
        ? TextRange.collapsed(_caret)
        : _composing;
    final composed = '${_text.substring(word.start, word.end)}$character';
    final before = _text;
    _text = before.replaceRange(word.start, word.end, composed);
    _caret = word.start + composed.length;
    _composing = TextRange(start: word.start, end: _caret);
    return _send(
      TextEditingDeltaReplacement(
        oldText: before,
        replacementText: composed,
        replacedRange: word,
        selection: TextSelection.collapsed(offset: _caret),
        composing: _composing,
      ),
    );
  }

  Future<void> _send(TextEditingDelta delta) async {
    await sendDeltas(tester, [delta]);
    await _hearEditor();
  }

  /// Restart for every state the editor sent since the keyboard last
  /// listened.
  Future<void> _hearEditor() async {
    final log = tester.testTextInput.log;
    final sent = [
      for (final call in log.skip(_heard))
        if (call.method == 'TextInput.setEditingState')
          call.arguments as Map<Object?, Object?>,
    ];
    _heard = log.length;

    if (sent.isEmpty || restarts >= _giveUpAfter) {
      return;
    }

    restarts += sent.length;
    final state = sent.last;
    await _restart(state['text']! as String, state['selectionExtent']! as int);
  }

  Future<void> _restart(String text, int caret) async {
    _text = text;
    _caret = caret;
    // The word before the caret, after the editor's line-break prefix.
    final start = text.lastIndexOf(RegExp(r'[\s]'), caret - 1) + 1;
    _composing = start < caret
        ? TextRange(start: start, end: caret)
        : TextRange.empty;

    if (!_composing.isCollapsed) {
      await _send(
        TextEditingDeltaNonTextUpdate(
          oldText: _text,
          selection: TextSelection.collapsed(offset: _caret),
          composing: _composing,
        ),
      );
    }
  }
}

Future<void> _runsKeyBindings(WidgetTester tester) async {
  final controller = await _focused(tester, 'bold me');
  final kit = starterKit.kit;
  controller.selectAll();
  await tester.pump();

  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  expect(controller.query(kit.bold.isBold()), isTrue);
  expect(controller.export(Format.markdown), '**bold me**');

  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.keyZ);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  expect(controller.export(Format.markdown), 'bold me');

  await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
  await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
  expect(controller.export(Format.text), 'bold m');
  await tester.sendKeyEvent(LogicalKeyboardKey.enter);
  expect(controller.export(Format.text), 'bold m\n');
}

Future<void> _copiesCutsAndPastes(WidgetTester tester) async {
  final clipboard = <String>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    SystemChannels.platform,
    (call) async {
      if (call.method == 'Clipboard.setData') {
        clipboard.add(
          (call.arguments as Map<Object?, Object?>)['text']! as String,
        );
      } else if (call.method == 'Clipboard.getData') {
        return {'text': clipboard.last};
      }
      return null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    ),
  );
  final controller = await _focused(tester, '**strong** words');
  controller.select(const Position([0], 0), const Position([0], 6));
  await tester.pump();

  Future<void> chord(LogicalKeyboardKey key) async {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(key);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
  }

  await chord(LogicalKeyboardKey.keyX);
  expect(clipboard, ['strong']);
  expect(controller.export(Format.text), ' words');

  controller.select(const Position([0], 6));
  await chord(LogicalKeyboardKey.keyV);
  expect(controller.export(Format.markdown), ' words**strong**');
}

Future<void> _selectsWithAMouse(WidgetTester tester) async {
  final controller = await _focused(tester, 'alpha beta gamma');
  final origin = tester.getTopLeft(find.byType(CanvistEditor));
  Offset at(int character) =>
      origin + Offset(_padding + character * _em + 1, _padding + _em / 2);

  final mouse = await tester.startGesture(at(6), kind: PointerDeviceKind.mouse);
  await mouse.moveTo(at(10));
  await mouse.up();
  await tester.pump();
  expect(controller.view.copySelection()?.text, 'beta');

  await tester.pump(const Duration(seconds: 1));

  for (var click = 0; click < 2; click++) {
    final down = await tester.startGesture(
      at(13),
      kind: PointerDeviceKind.mouse,
    );
    await down.up();
    await tester.pump(const Duration(milliseconds: 50));
  }

  expect(controller.view.copySelection()?.text, 'gamma');
}

Future<void> _selectsWithTouch(WidgetTester tester) async {
  final controller = await _focused(tester, 'alpha beta gamma');
  final origin = tester.getTopLeft(find.byType(CanvistEditor));

  await tester.longPressAt(
    origin + const Offset(_padding + 7 * _em, _padding + _em / 2),
  );
  await tester.pumpAndSettle();
  expect(controller.view.copySelection()?.text, 'beta');
  expect(find.text('Copy'), findsOneWidget);
  expect(find.text('Cut'), findsOneWidget);

  await tester.tap(find.text('Copy'));
  await tester.pumpAndSettle();
  expect(find.text('Copy'), findsNothing);
}

Future<void> _scrollsToTheCaret(WidgetTester tester) async {
  final controller = await _focused(
    tester,
    List.generate(_paragraphs, (index) => 'Paragraph $index').join('\n\n'),
    size: const Size(400, 300),
  );
  final scrollable = tester.state<ScrollableState>(find.byType(Scrollable));
  expect(scrollable.position.maxScrollExtent, greaterThan(1000));

  controller.select(const Position([_paragraphs - 1], 3));
  await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
  await tester.pump();
  await tester.pump();
  expect(
    scrollable.position.pixels,
    greaterThan(1000),
    reason: 'scrolled to the caret',
  );

  await tester.drag(find.byType(CanvistEditor), const Offset(0, 400));
  await tester.pump(kDoubleTapTimeout);
  expect(
    scrollable.position.pixels,
    lessThan(scrollable.position.maxScrollExtent),
  );
}

Future<void> _exposesSemantics(WidgetTester tester) async {
  final handle = tester.ensureSemantics();
  final controller = controllerFor('# Hi\n\nthere');
  await pumpEditor(
    tester,
    CanvistEditor(
      controller: controller,
      autofocus: true,
      semanticsLabel: 'Notes',
    ),
  );
  await tester.pump();
  controller.select(const Position([1], 0), const Position([1], 5));
  await tester.pump();

  final node = tester.getSemantics(find.bySemanticsLabel('Notes'));
  expect(
    node,
    matchesSemantics(
      label: 'Notes',
      isTextField: true,
      isMultiline: true,
      isFocused: true,
      isFocusable: true,
      hasTapAction: true,
      hasCopyAction: true,
      hasCutAction: true,
      hasPasteAction: true,
      hasSetSelectionAction: true,
      hasSetTextAction: true,
      hasMoveCursorForwardByCharacterAction: true,
      hasMoveCursorBackwardByCharacterAction: true,
      value: 'Hi\nthere',
    ),
  );
  expect(
    node.getSemanticsData().textSelection,
    const TextSelection(baseOffset: 3, extentOffset: 8),
  );
  handle.dispose();
}

Future<void> _setsTextAsAssistiveTechnologyDoes(WidgetTester tester) async {
  final handle = tester.ensureSemantics();
  final controller = controllerFor('# Hi\n\nthere');
  await pumpEditor(
    tester,
    CanvistEditor(
      controller: controller,
      autofocus: true,
      semanticsLabel: 'Notes',
    ),
  );
  await tester.pump();

  tester.semantics.setText(
    find.semantics.byLabel('Notes'),
    'Hi\nthere you\nand more',
  );
  await tester.pump();

  expect(controller.export(Format.text), 'Hi\nthere you\nand more');
  expect(
    controller.export(Format.markdown),
    startsWith('# Hi\n\n'),
    reason: 'only the changed text is replaced, so the heading stays',
  );
  handle.dispose();
}

Future<void> _collaborates(WidgetTester tester) async {
  final alice = controllerFor('Shared');
  alice.doc.share(1);
  final bobDoc = starterKit.engine.fromYjs(
    alice.doc.encodeState(),
    clientId: 2,
  );
  final bob = CanvistController(engine: starterKit.engine, doc: bobDoc);
  addTearDown(() {
    bob.dispose();
    bobDoc.dispose();
  });

  await pumpEditor(
    tester,
    Column(
      children: [
        Expanded(child: CanvistEditor(controller: alice, autofocus: true)),
        Expanded(child: CanvistEditor(controller: bob)),
      ],
    ),
  );
  await tester.pump();
  alice.select(const Position([0], 6));
  await tester.pump();
  await sendDeltas(tester, [typed(alice, ' doc')]);

  for (final update in alice.doc.takeUpdates()) {
    bob.applyUpdate(update);
  }

  await tester.pump();

  expect(bob.export(Format.text), 'Shared doc');
  final runs = bob.view.frame(0, 300).display;
  expect(runs, isNull, reason: 'the editor already painted the update');
}

Future<void> _keepsUpWithFastTyping(WidgetTester tester) async {
  final controller = await _focused(tester, '');
  final random = Random(7);
  final keys = [
    LogicalKeyboardKey.enter,
    LogicalKeyboardKey.backspace,
    LogicalKeyboardKey.arrowLeft,
    LogicalKeyboardKey.arrowUp,
  ];
  final watch = Stopwatch();
  final frames = <int>[];

  for (var index = 0; index < _keystrokes; index++) {
    watch
      ..reset()
      ..start();

    if (index % _keyEvery == 0) {
      await tester.sendKeyEvent(keys[random.nextInt(keys.length)]);
    } else {
      final letter = _alphabet[random.nextInt(_alphabet.length)];
      await sendDeltas(tester, [typed(controller, letter)]);
    }

    frames.add(watch.elapsedMicroseconds);
  }

  frames.sort();
  final median = frames[frames.length ~/ 2];
  // A keystroke on the host, from the platform message to a painted frame.
  final slack =
      double.tryParse(Platform.environment['CANVIST_PERF_SLACK'] ?? '') ?? 1;
  expect(
    median,
    lessThan(_frameMicroseconds * slack),
    reason: 'median keystroke $median µs',
  );
  expect(controller.export(Format.text), isNotEmpty);
}
