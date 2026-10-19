import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:canvist_flutter_example/main.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'ffi_performance.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  late Engine engine;
  late StarterKitHandles kit;

  setUpAll(() async {
    (:engine, :kit) = await Engine.starterKit();
  });

  group('performance', () => ffiPerformanceTests(() => engine));

  group('Doc', () {
    test('starts with one empty paragraph', () {
      expect(engine.create().export(Format.html), '<p></p>');
    });

    test('runs typed commands through Rust', () {
      final doc = engine.parse(Format.markdown, 'hello world')
        ..select(const Position([0], 6), const Position([0], 11))
        ..run(kit.bold.toggleBold());
      expect(doc.export(Format.markdown), 'hello **world**');
      expect(doc.query(kit.bold.isBold()), isTrue);
    });

    test('offsets count UTF-16 code units', () {
      final doc = engine.parse(Format.text, 'h😀llo')
        ..select(const Position([0], 3))
        ..insertText('—');
      expect(doc.export(Format.text), 'h😀—llo');
    });

    test('rejects positions that split a character', () {
      final doc = engine.parse(Format.text, 'h😀llo');
      expect(
        () => doc.select(const Position([0], 2)),
        throwsA(
          isA<CanvistException>().having(
            (error) => error.code,
            'code',
            ErrorCode.invalidPosition,
          ),
        ),
      );
    });
  });

  testWidgets('appending text round-trips through Rust', (tester) async {
    await tester.pumpWidget(CanvistExampleApp(engine: engine, kit: kit));
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

    await tester.tap(find.byKey(const Key('bold')));
    await tester.pump();
    expect(tester.widget<Text>(document).data, '**hello world**');
  });
}
