/// The example app on a device: the engine through the bridge, then the
/// editor itself: typing, input rules, IME composition, touch selection,
/// the toolbar, undo, collaboration, and keystroke-to-frame time.
library;

import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:canvist_flutter_example/main.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'ffi_performance.dart';
import 'text_input.dart';

/// Slow devices and CI emulators get this much more time per keystroke.
final _slack =
    double.tryParse(const String.fromEnvironment('CANVIST_PERF_SLACK')) ?? 1;

/// The example app pads its document by this much.
const _padding = 24.0;

/// One 60 Hz frame.
const _frameMicroseconds = 16667;

/// How long the two-peer relay takes, with room to spare.
const _relay = Duration(milliseconds: 300);

/// Fast typing: how long the document is, how many keys, and how often a
/// word ends or a key deletes.
const _longDocument = 400;
const _keystrokes = 300;
const _wordEvery = 7;
const _deleteEvery = 13;

late Engine _engine;
late StarterKitHandles _kit;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  CanvistEditor.debugDeterministicCaret = true;

  setUpAll(() async {
    final (:engine, :kit) = await Engine.starterKit(
      platform: CanvistController.keyPlatform,
    );
    _engine = engine;
    _kit = kit;
  });

  group('performance', () => ffiPerformanceTests(() => _engine));

  group('Doc', () {
    test('runs typed commands through Rust', _runsTypedCommands);
    test('offsets count UTF-16 code units', _countsUtf16Offsets);
  });

  group('editor', () {
    testWidgets('typing goes through the platform text input', _types);
    testWidgets('input rules turn markup into blocks', _runsInputRules);
    testWidgets('IME composition commits as one undo step', _composes);
    testWidgets(
      'a long press selects a word and the toolbar formats it',
      _selectsWithTouch,
    );
    testWidgets('formatting buttons follow the selection', _formats);
    testWidgets('two peers edit one document', _collaborates);
    testWidgets(
      'a keystroke reaches the screen within a frame, and fast typing never fails',
      _keepsUpWithFastTyping,
    );
  });
}

void _runsTypedCommands() {
  final doc = _engine.parse(Format.markdown, 'hello world')
    ..select(const Position([0], 6), const Position([0], 11))
    ..run(_kit.bold.toggleBold());

  expect(doc.export(Format.markdown), 'hello **world**');
  expect(doc.query(_kit.bold.isBold()), isTrue);
}

void _countsUtf16Offsets() {
  final doc = _engine.parse(Format.text, 'h😀llo')
    ..select(const Position([0], 3))
    ..insertText('—');

  expect(doc.export(Format.text), 'h😀—llo');
}

/// Open the example app on [markdown] and return its editor's controller.
///
/// The platform's text input is replaced by the test's, which speaks the
/// same channel protocol: a simulator cannot drive its software keyboard or
/// input methods, so the tests send what they would.
Future<CanvistController> _open(WidgetTester tester, String markdown) async {
  tester.testTextInput.register();
  addTearDown(tester.testTextInput.unregister);
  await tester.pumpWidget(
    CanvistExampleApp(engine: _engine, kit: _kit, markdown: markdown),
  );
  await tester.pumpAndSettle();

  return tester
      .widget<CanvistEditor>(find.byKey(const Key('editor')))
      .controller;
}

/// The controller of the editor inside the widget keyed [key].
CanvistController _editorIn(WidgetTester tester, String key) => tester
    .widget<CanvistEditor>(
      find.descendant(
        of: find.byKey(Key(key)),
        matching: find.byType(CanvistEditor),
      ),
    )
    .controller;

Future<void> _tap(WidgetTester tester, String key) async {
  await tester.tap(find.byKey(Key(key)));
  await tester.pump();
}

String _markdown(CanvistController controller) =>
    controller.export(Format.markdown);

Future<void> _types(WidgetTester tester) async {
  final controller = await _open(tester, '');
  await sendDeltas(tester, [typed(controller, 'Hello')]);
  await sendDeltas(tester, [typed(controller, '\nworld')]);

  expect(controller.export(Format.text), 'Hello\nworld');
  expect(tester.testTextInput.editingState?['text'], '\nworld');
}

Future<void> _runsInputRules(WidgetTester tester) async {
  final controller = await _open(tester, '');

  for (final character in '## Plan'.split('')) {
    await sendDeltas(tester, [typed(controller, character)]);
  }

  expect(_markdown(controller), '## Plan');
}

Future<void> _composes(WidgetTester tester) async {
  final controller = await _open(tester, 'Say');
  controller
    ..select(const Position([0], 3))
    ..insertText(' ');
  await tester.pump();

  await sendDeltas(tester, [
    typed(controller, 'に', composing: const TextRange(start: 5, end: 6)),
  ]);
  expect(controller.view.editState?.composingStart, 4);

  await sendDeltas(tester, [
    TextEditingDeltaReplacement(
      oldText: '\nSay に',
      replacementText: '日本',
      replacedRange: const TextRange(start: 5, end: 6),
      selection: const TextSelection.collapsed(offset: 7),
      composing: TextRange.empty,
    ),
  ]);
  expect(controller.export(Format.text), 'Say 日本');

  await _tap(tester, 'undo');
  expect(controller.export(Format.text), isNot(contains('日本')));
  expect(controller.export(Format.text), startsWith('Say'));
}

Future<void> _selectsWithTouch(WidgetTester tester) async {
  final controller = await _open(tester, 'alpha beta gamma');
  final editor = tester.getRect(find.byKey(const Key('editor')));
  // Aim inside "beta", found from where the caret would be drawn there.
  controller.select(const Position([0], 8));
  await tester.pump();
  final caret = controller.view.caretRect!;
  final inBeta = Offset(caret.x, caret.y + caret.height / 2);

  await tester.longPressAt(
    editor.topLeft + const Offset(_padding, _padding) + inBeta,
  );
  await tester.pumpAndSettle();
  expect(controller.view.copySelection()?.text, 'beta');
  expect(find.text('Copy'), findsOneWidget);

  // On a phone the menu sits over the formatting toolbar: dismiss it first.
  ContextMenuController.removeAny();
  await tester.pump();
  await _tap(tester, 'bold');
  expect(_markdown(controller), 'alpha **beta** gamma');
}

Future<void> _formats(WidgetTester tester) async {
  final controller = await _open(tester, 'Make me a heading');
  controller.selectAll();
  await tester.pump();

  await _tap(tester, 'heading1');
  expect(_markdown(controller), '# Make me a heading');
  final heading = tester.widget<IconButton>(find.byKey(const Key('heading1')));
  expect(heading.isSelected, isTrue);

  await _tap(tester, 'italic');
  expect(_markdown(controller), '# *Make me a heading*');

  await _tap(tester, 'undo');
  expect(_markdown(controller), '# Make me a heading');

  await _tap(tester, 'redo');
  expect(_markdown(controller), '# *Make me a heading*');
}

Future<void> _collaborates(WidgetTester tester) async {
  final controller = await _open(tester, 'Shared');
  await tester.tap(find.byKey(const Key('peers')));
  await tester.pumpAndSettle();

  final you = _editorIn(tester, 'you');
  final peer = _editorIn(tester, 'peer');
  expect(identical(you, controller), isTrue);

  you.select(const Position([0], 6));
  await tester.pump();
  await sendDeltas(tester, [typed(you, ' notes')]);
  await tester.pump(_relay);
  expect(peer.export(Format.text), 'Shared notes');

  peer
    ..select(const Position([0], 0))
    ..insertText('Our ');
  await tester.pump(_relay);
  expect(you.export(Format.text), 'Our Shared notes');

  await tester.tap(find.byKey(const Key('peers')));
  await tester.pumpAndSettle();
}

Future<void> _keepsUpWithFastTyping(WidgetTester tester) async {
  final paragraph = 'A paragraph of text that wraps on a phone screen.';
  final controller = await _open(
    tester,
    List.filled(_longDocument, paragraph).join('\n\n'),
  );
  controller.select(const Position([3], 0));
  await tester.pump();

  final input = <int>[];
  final frames = <FrameTiming>[];
  void collect(List<FrameTiming> timings) => frames.addAll(timings);
  SchedulerBinding.instance.addTimingsCallback(collect);

  for (var index = 1; index <= _keystrokes; index++) {
    final watch = Stopwatch()..start();
    await _press(tester, controller, index);
    input.add(watch.elapsedMicroseconds);
    await tester.pump();
  }

  // The engine reports frame timings in batches.
  await tester.binding.delayed(const Duration(seconds: 2));
  SchedulerBinding.instance.removeTimingsCallback(collect);

  final work = _median(input);
  final build = _median([
    for (final timing in frames) timing.buildDuration.inMicroseconds,
  ]);
  final raster = _median([
    for (final timing in frames) timing.rasterDuration.inMicroseconds,
  ]);
  final binding = IntegrationTestWidgetsFlutterBinding.instance;
  binding.reportData = {
    ...?binding.reportData,
    'keystroke_input_median_us': work,
    'keystroke_build_median_us': build,
    'keystroke_raster_median_us': raster,
  };
  debugPrint(
    'PERF flutter keystroke input median $work µs, '
    'build median $build µs, raster median $raster µs',
  );

  // A keystroke's work on the UI thread, from the platform message through
  // Rust to the frame that paints it, fits in one 60 Hz frame. Raster time
  // is reported but not budgeted: it measures the device's GPU, and CI
  // emulators render in software. Debug builds run Dart in JIT mode, so
  // devices get slack.
  expect(frames, isNotEmpty);
  expect(
    work + build,
    lessThan(_frameMicroseconds * _slack),
    reason: 'input $work µs + build $build µs',
  );
  expect(controller.export(Format.text), contains('x'));
}

int _median(List<int> values) => (values..sort())[values.length ~/ 2];

/// Keystroke [index] of fast typing, without pumping a frame: mostly
/// letters, with spaces between words and the odd Backspace.
Future<void> _press(
  WidgetTester tester,
  CanvistController controller,
  int index,
) async {
  if (index % _deleteEvery == 0) {
    await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
    return;
  }

  final character = index % _wordEvery == 0 ? ' ' : 'x';
  await deliverDeltas(tester, [typed(controller, character)]);
}
