/// The example editor driven through the device itself: the platform's
/// keyboard and accessibility, native taps and swipes, long presses, the
/// toolbar, two collaborating peers, and the system's dark mode.
library;

import 'dart:io';

import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:canvist_flutter_example/keys.dart';
import 'package:canvist_flutter_example/main.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart'
    show expect, greaterThan, isTrue, lessThan;
import 'package:patrol/patrol.dart';

/// The example app pads its document by this much.
const _padding = 24.0;

/// How long the two-peer relay takes, with room to spare.
const _relay = Duration(milliseconds: 400);

/// A long run of fast typing through the platform's text input.
const _burst =
    'The quick brown fox jumps over the lazy dog while the editor keeps up '
    'with every character, word after word, without dropping or doubling '
    'any of them on the way from the keyboard to the Rust core and back.';

/// Patrol 4.10 types on iOS only into single-line text fields
/// (`XCUIElementTypeTextField`), and the editor is a multiline text view; its
/// Settings automation for dark mode leaves the appearance unchanged on the
/// iOS 26 simulator. These tests run on Android, and the iOS text input path
/// is covered by the integration tests through the text input channel.
final _androidOnly = Platform.isIOS;

void main() {
  patrolTest(
    'the platform keyboard types into the editor',
    skip: _androidOnly,
    _types,
  );
  patrolTest(
    'fast typing keeps every character',
    skip: _androidOnly,
    _typesFast,
  );
  patrolTest('a native tap places the caret where it lands', _tapsCaret);
  patrolTest('a long press selects a word to format', _selectsWord);
  patrolTest('the toolbar formats, undoes, and redoes', _formats);
  patrolTest('two peers edit one document', skip: _androidOnly, _collaborates);
  patrolTest('a native swipe scrolls a long document', _scrolls);
  patrolTest(
    'the editor follows the system dark mode',
    skip: _androidOnly,
    _followsDarkMode,
  );
}

/// Open the example app on [markdown] and return its editor's controller.
Future<CanvistController> _open(
  PatrolIntegrationTester $,
  String markdown,
) async {
  final (:engine, :kit) = await Engine.starterKit(
    platform: CanvistController.keyPlatform,
  );
  await $.pumpWidgetAndSettle(
    CanvistExampleApp(engine: engine, kit: kit, markdown: markdown),
  );

  return _controller($, Keys.editor);
}

/// The controller of the editor keyed [key], or inside the widget keyed
/// [key].
CanvistController _controller(PatrolIntegrationTester $, Key key) {
  final editor = $(key).$(CanvistEditor).exists
      ? $(key).$(CanvistEditor)
      : $(key);
  return $.tester.widget<CanvistEditor>(editor.finder.first).controller;
}

/// Where position [at] is drawn on screen, as the 0–1 fractions native taps
/// take.
Offset _onScreen(
  PatrolIntegrationTester $,
  CanvistController controller,
  Position at,
) {
  controller.select(at);
  final caret = controller.view.caretRect!;
  final editor = $.tester.getTopLeft($(Keys.editor).finder);
  final screen = $.tester.view.physicalSize / $.tester.view.devicePixelRatio;
  final point =
      editor +
      Offset(_padding + caret.x, _padding + caret.y + caret.height / 2);

  return Offset(point.dx / screen.width, point.dy / screen.height);
}

String _text(CanvistController controller) => controller.export(Format.text);

String _markdown(CanvistController controller) =>
    controller.export(Format.markdown);

Future<void> _types(PatrolIntegrationTester $) async {
  final controller = await _open($, 'Hello');

  await $.platform.mobile.enterTextByIndex('Hello from Patrol', index: 0);
  await $.pump(_relay);

  expect(_text(controller), 'Hello from Patrol');
}

Future<void> _typesFast(PatrolIntegrationTester $) async {
  final controller = await _open($, '');

  await $.platform.mobile.enterTextByIndex(_burst, index: 0);
  await $.pump(_relay);

  expect(_text(controller), _burst);
}

Future<void> _tapsCaret(PatrolIntegrationTester $) async {
  final controller = await _open($, 'alpha beta gamma');
  final target = _onScreen($, controller, const Position([0], 8));
  controller.select(const Position([0], 0));
  await $.pump();

  await $.platform.mobile.tapAt(target);
  await $.pump(_relay);

  final head = controller.selection!.head;
  expect(head.path, [0]);
  expect((head.offset - 8).abs() <= 1, isTrue, reason: 'caret at $head');
}

Future<void> _selectsWord(PatrolIntegrationTester $) async {
  final controller = await _open($, 'alpha beta gamma');
  final editor = $.tester.getTopLeft($(Keys.editor).finder);
  controller.select(const Position([0], 8));
  await $.pump();
  final caret = controller.view.caretRect!;

  await $.tester.longPressAt(
    editor + Offset(_padding + caret.x, _padding + caret.y + caret.height / 2),
  );
  await $.pumpAndSettle();
  await $('Copy').waitUntilVisible();
  expect(controller.view.copySelection()?.text, 'beta');

  // On a phone the menu sits over the formatting toolbar: dismiss it first.
  ContextMenuController.removeAny();
  await $(Keys.toolbar('bold')).tap();

  expect(_markdown(controller), 'alpha **beta** gamma');
}

Future<void> _formats(PatrolIntegrationTester $) async {
  final controller = await _open($, 'Make me a heading');
  controller.selectAll();

  await $(Keys.toolbar('heading1')).tap();
  await $(Keys.toolbar('italic')).tap();
  final formatted = _markdown(controller);
  await $(Keys.toolbar('undo')).tap();
  final undone = _markdown(controller);
  await $(Keys.toolbar('redo')).tap();

  expect(formatted, '# *Make me a heading*');
  expect(undone, '# Make me a heading');
  expect(_markdown(controller), '# *Make me a heading*');
}

Future<void> _collaborates(PatrolIntegrationTester $) async {
  await _open($, 'Shared');
  await $(Keys.peers).tap();
  final you = _controller($, Keys.you);
  final peer = _controller($, Keys.peer);

  await $.platform.mobile.enterTextByIndex('Shared notes', index: 0);
  await $.pump(_relay);
  peer
    ..select(const Position([0], 0))
    ..insertText('Our ');
  await $.pump(_relay);

  expect(_text(peer), 'Our Shared notes');
  expect(_text(you), 'Our Shared notes');
}

Future<void> _scrolls(PatrolIntegrationTester $) async {
  final paragraph = 'A paragraph of text that wraps on a phone screen.';
  await _open($, List.filled(200, paragraph).join('\n\n'));
  final scrollable = $.tester.state<ScrollableState>(
    $(Keys.editor).$(Scrollable).finder,
  );

  // The editor is focused, so start above a phone's on-screen keyboard.
  await $.platform.mobile.swipe(
    from: const Offset(0.5, 0.55),
    to: const Offset(0.5, 0.3),
  );
  await $.pumpAndSettle();

  expect(scrollable.position.pixels, greaterThan(0));
}

Future<void> _followsDarkMode(PatrolIntegrationTester $) async {
  await _open($, 'Light, then dark');
  final light = await _backgroundLuminance($);

  await $.platform.mobile.enableDarkMode();
  await $.pumpAndSettle();
  final dark = await _backgroundLuminance($);
  await $.platform.mobile.disableDarkMode();

  expect(light, greaterThan(200), reason: 'a light page in light mode');
  expect(dark, lessThan(80), reason: 'a dark page in dark mode');
}

/// The brightness, 0 to 255, of a pixel the editor paints below its text:
/// read from what the screen draws, not from the theme.
Future<double> _backgroundLuminance(PatrolIntegrationTester $) async {
  final rect = $.tester.getRect($(Keys.editor).finder);
  final point = Offset(rect.center.dx, rect.bottom - _padding);
  final root = $.tester.binding.renderViews.first.debugLayer! as OffsetLayer;
  final image = await root.toImage(
    Rect.fromCenter(center: point, width: 1, height: 1),
  );
  final pixel = (await image.toByteData())!;
  image.dispose();

  return 0.2126 * pixel.getUint8(0) +
      0.7152 * pixel.getUint8(1) +
      0.0722 * pixel.getUint8(2);
}
