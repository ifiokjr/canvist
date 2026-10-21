/// [EditorInput] alone: how platform text input becomes editor calls.
library;

import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

void main() {
  setUpAll(loadStarterKit);

  ({Doc doc, EditorInput input}) open(String text) {
    final doc = starterKit.engine.parse(Format.text, text);
    final view = doc.view(const FixedMeasurer())..setWidth(400);
    addTearDown(() {
      view.dispose();
      doc.dispose();
    });
    return (doc: doc, input: EditorInput(view));
  }

  TextEditingDelta insert(TextEditingValue value, String text) {
    final caret = value.selection.extentOffset;
    return TextEditingDeltaInsertion(
      oldText: value.text,
      textInserted: text,
      insertionOffset: caret,
      selection: TextSelection.collapsed(offset: caret + text.length),
      composing: TextRange.empty,
    );
  }

  test('the platform edits the caret block behind a line break', () {
    final (:doc, :input) = open('one\ntwo');
    doc.select(const Position([1], 2));

    expect(
      input.value,
      const TextEditingValue(
        text: '\ntwo',
        selection: TextSelection.collapsed(offset: 3),
      ),
    );
  });

  test('typed text lands at the caret, and line breaks split blocks', () {
    final (:doc, :input) = open('');

    expect(input.applyDeltas([insert(input.value, 'Hello')]), isTrue);
    expect(doc.export(Format.text), 'Hello');
    expect(input.value.selection, const TextSelection.collapsed(offset: 6));

    input.applyDeltas([insert(input.value, '\nworld')]);
    expect(doc.export(Format.text), 'Hello\nworld');
    expect(input.value.text, '\nworld');
  });

  test('whole values become the one edit between them', () {
    const before = TextEditingValue(text: '\nabc');
    const after = TextEditingValue(text: '\naXYc');
    final delta =
        EditorInput.delta(before, after) as TextEditingDeltaReplacement;

    expect(delta.replacedRange, const TextRange(start: 2, end: 3));
    expect(delta.replacementText, 'XY');
  });
}
