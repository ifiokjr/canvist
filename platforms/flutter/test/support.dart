import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

/// The StarterKit engine, built once for every test in a file.
late final ({Engine engine, StarterKitHandles kit}) starterKit;

Future<void> loadStarterKit() async {
  starterKit = await Engine.starterKit();
}

/// A controller for [content] read as [format], disposed when the test
/// ends.
CanvistController controllerFor(
  String content, {
  Format format = Format.markdown,
  CanvistFonts fonts = const CanvistFonts(),
}) {
  final controller = CanvistController(
    engine: starterKit.engine,
    doc: starterKit.engine.parse(format, content),
    fonts: fonts,
  );
  addTearDown(() {
    controller.dispose();
    controller.doc.dispose();
  });
  return controller;
}

/// Pump an app holding [editor] in a box [size] big.
Future<void> pumpEditor(
  WidgetTester tester,
  Widget editor, {
  Size size = const Size(400, 600),
  ThemeData? theme,
}) async {
  CanvistEditor.debugDeterministicCaret = true;
  addTearDown(() => CanvistEditor.debugDeterministicCaret = false);
  await tester.pumpWidget(
    MaterialApp(
      theme: theme,
      home: Scaffold(
        body: Align(
          alignment: Alignment.topLeft,
          child: SizedBox.fromSize(size: size, child: editor),
        ),
      ),
    ),
  );
}

/// The id of the text input client the editor attached last.
int clientId(WidgetTester tester) {
  final call = tester.testTextInput.log.lastWhere(
    (call) => call.method == 'TextInput.setClient',
  );
  return (call.arguments as List<Object?>).first! as int;
}

/// Send [deltas] to the editor as the platform's delta model does.
Future<void> sendDeltas(
  WidgetTester tester,
  List<TextEditingDelta> deltas,
) async {
  await deliverDeltas(tester, deltas);
  await tester.pump();
}

/// Hand [deltas] to the editor without pumping a frame.
Future<void> deliverDeltas(
  WidgetTester tester,
  List<TextEditingDelta> deltas,
) async {
  final message = SystemChannels.textInput.codec.encodeMethodCall(
    MethodCall('TextInputClient.updateEditingStateWithDeltas', [
      clientId(tester),
      {
        'deltas': [for (final delta in deltas) _deltaJson(delta)],
      },
    ]),
  );
  await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
    SystemChannels.textInput.name,
    message,
    (_) {},
  );
}

Map<String, Object?> _deltaJson(TextEditingDelta delta) {
  final (start, end, text) = switch (delta) {
    TextEditingDeltaInsertion() => (
      delta.insertionOffset,
      delta.insertionOffset,
      delta.textInserted,
    ),
    TextEditingDeltaDeletion() => (
      delta.deletedRange.start,
      delta.deletedRange.end,
      '',
    ),
    TextEditingDeltaReplacement() => (
      delta.replacedRange.start,
      delta.replacedRange.end,
      delta.replacementText,
    ),
    _ => (-1, -1, ''),
  };

  return {
    'oldText': delta.oldText,
    'deltaText': text,
    'deltaStart': start,
    'deltaEnd': end,
    'selectionBase': delta.selection.baseOffset,
    'selectionExtent': delta.selection.extentOffset,
    'selectionAffinity': 'TextAffinity.downstream',
    'selectionIsDirectional': false,
    'composingBase': delta.composing.start,
    'composingExtent': delta.composing.end,
  };
}

/// An insertion of [text] at the caret, as the platform would send it.
/// The platform's value is the editor's own: `TestTextInput` never applies
/// deltas, so its record of the value goes stale after the first one.
TextEditingDeltaInsertion typed(
  CanvistController controller,
  String text, {
  TextRange composing = TextRange.empty,
}) {
  final value = EditorInput(controller.view).value;
  final caret = value.selection.extentOffset;

  return TextEditingDeltaInsertion(
    oldText: value.text,
    textInserted: text,
    insertionOffset: caret,
    selection: TextSelection.collapsed(offset: caret + text.length),
    composing: composing,
  );
}
