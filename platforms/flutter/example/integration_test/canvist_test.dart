import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:canvist_flutter_example/main.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'ffi_performance.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustLib.init);

  group('performance', ffiPerformanceTests);

  group('EditorHandle', () {
    test('starts empty', () {
      expect(EditorHandle().text(), '');
    });

    test('inserts text through Rust', () {
      final editor = EditorHandle()
        ..insertText(offset: 0, text: 'world')
        ..insertText(offset: 0, text: 'hello ');
      expect(editor.text(), 'hello world');
    });

    test('offsets count Unicode scalar values', () {
      final editor = EditorHandle()
        ..insertText(offset: 0, text: 'h😀llo')
        ..insertText(offset: 2, text: '—');
      expect(editor.text(), 'h😀—llo');
    });

    test('rejects offsets past the end', () {
      final editor = EditorHandle();
      expect(
        () => editor.insertText(offset: 1, text: 'x'),
        throwsA(const OffsetOutOfBounds(offset: 1, length: 0)),
      );
      expect(editor.text(), '');
    });

    test('reports the Rust core version', () {
      expect(EditorHandle().version(), matches(RegExp(r'^\d+\.\d+\.\d+')));
    });
  });

  testWidgets('appending text round-trips through Rust', (tester) async {
    await tester.pumpWidget(const CanvistExampleApp());
    final document = find.byKey(const Key('document'));
    expect(tester.widget<Text>(document).data, 'Empty');

    await tester.enterText(find.byKey(const Key('draft')), 'hello');
    await tester.pump();
    await tester.tap(find.byKey(const Key('append')));
    await tester.pump();
    expect(tester.widget<Text>(document).data, 'hello');

    await tester.enterText(find.byKey(const Key('draft')), ' world');
    await tester.pump();
    await tester.tap(find.byKey(const Key('append')));
    await tester.pump();
    expect(tester.widget<Text>(document).data, 'hello world');
  });
}
