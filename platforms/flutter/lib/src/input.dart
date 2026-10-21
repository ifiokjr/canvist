import 'dart:math' as math;

import 'package:canvist/canvist.dart';
import 'package:flutter/services.dart';

/// Keeps a platform text input and an [EditorView] in step.
///
/// The platform edits one text block at a time: the block holding the
/// selection's head, behind a [prefix] line break. Deleting the prefix is
/// Backspace at the start of the block, so soft keyboards, which send no
/// key events, still join blocks and lift list items; and the line break
/// before the text tells keyboards a paragraph starts there, so they
/// capitalize it. A typed line break splits the block, as Enter does.
///
/// Composition follows the platform's composing range: text an input
/// method composes becomes the editor's preedit, which collaborators and
/// history only see once it is committed.
final class EditorInput {
  EditorInput(this.view);

  /// What the platform's text sits behind.
  static const prefix = '\n';

  /// The view the platform edits.
  EditorView view;

  /// The editing value to give the platform for the current selection.
  TextEditingValue get value {
    final state = view.editState;

    if (state == null) {
      return const TextEditingValue(
        text: prefix,
        selection: TextSelection.collapsed(offset: 1),
      );
    }

    final composing = switch ((state.composingStart, state.composingEnd)) {
      (final int start, final int end) => TextRange(
        start: start + 1,
        end: end + 1,
      ),
      _ => TextRange.empty,
    };

    return TextEditingValue(
      text: '$prefix${state.text}',
      selection: TextSelection(
        baseOffset: state.anchor + 1,
        extentOffset: state.head + 1,
      ),
      composing: composing,
    );
  }

  /// Whether an input method is composing text now.
  bool get composing => view.editState?.composingStart != null;

  /// Apply what the platform changed, in order. Returns whether the
  /// document or selection changed.
  bool applyDeltas(Iterable<TextEditingDelta> deltas) {
    var changed = false;

    for (final delta in deltas) {
      changed = _apply(delta) || changed;
    }

    return changed;
  }

  /// Apply a whole new value from a platform that sends no deltas, as the
  /// difference from [previous].
  bool applyValue(TextEditingValue previous, TextEditingValue next) =>
      _apply(delta(previous, next));

  /// The single edit that turns [previous] into [next]: everything between
  /// their common start and common end.
  static TextEditingDelta delta(
    TextEditingValue previous,
    TextEditingValue next,
  ) {
    final before = previous.text;
    final after = next.text;

    if (before == after) {
      return TextEditingDeltaNonTextUpdate(
        oldText: before,
        selection: next.selection,
        composing: next.composing,
      );
    }

    final shortest = math.min(before.length, after.length);
    var start = 0;

    while (start < shortest &&
        before.codeUnitAt(start) == after.codeUnitAt(start)) {
      start++;
    }

    var end = 0;

    while (end < shortest - start &&
        before.codeUnitAt(before.length - 1 - end) ==
            after.codeUnitAt(after.length - 1 - end)) {
      end++;
    }

    return TextEditingDeltaReplacement(
      oldText: before,
      replacementText: after.substring(start, after.length - end),
      replacedRange: TextRange(start: start, end: before.length - end),
      selection: next.selection,
      composing: next.composing,
    );
  }

  bool _apply(TextEditingDelta delta) {
    final changed = switch (delta) {
      TextEditingDeltaInsertion() => _replace(
        delta,
        TextRange.collapsed(delta.insertionOffset),
        delta.textInserted,
      ),
      TextEditingDeltaDeletion() => _replace(delta, delta.deletedRange, ''),
      TextEditingDeltaReplacement() => _replace(
        delta,
        delta.replacedRange,
        delta.replacementText,
      ),
      _ => _endComposition(delta.oldText, delta.composing),
    };
    final next = delta.apply(TextEditingValue(text: delta.oldText));

    return _select(next.text, delta.selection) || changed;
  }

  /// Replace [range] of the platform's text with [text].
  bool _replace(TextEditingDelta delta, TextRange range, String text) {
    if (range.start < prefix.length) {
      return _replaceAtStart(range, text);
    }

    final composing = delta.composing;

    if (composing.isValid && !composing.isCollapsed) {
      return _compose(delta, range, text);
    }

    final state = view.editState;

    if (state?.composingStart case final int start?) {
      final from = start + 1;
      final to = state!.composingEnd! + 1;

      if (range.start >= from && range.end <= to) {
        final old = delta.oldText;
        return view.commitComposition(
          old.substring(from, range.start) +
              text +
              old.substring(range.end, to),
        );
      }

      // An edit outside the composition ends it as it stands.
      view.commitComposition(delta.oldText.substring(from, to));
    }

    return _type(range.start - 1, range.end - 1, text);
  }

  /// An edit that reaches into the prefix: deleting it is Backspace at the
  /// start of the block.
  bool _replaceAtStart(TextRange range, String text) {
    final end = math.max(range.end - 1, 0);

    if (text.isNotEmpty) {
      return _type(0, end, text);
    }

    final deleted = end > 0 && view.replaceText(0, end, '');

    return view.keyDown('Backspace') || deleted;
  }

  /// Text the input method is composing: [range] becomes [text], and the
  /// composing range of the result is the preedit.
  bool _compose(TextEditingDelta delta, TextRange range, String text) {
    final region = delta.composing;
    final next = delta.apply(TextEditingValue(text: delta.oldText)).text;

    if (!composing) {
      // A new composition, perhaps over text the input method marked: select
      // that text as it was before this edit, for the preedit to replace.
      final growth = text.length - (range.end - range.start);
      final start = math.min(region.start, range.start);
      final end = math.max(region.end - growth, range.end);
      view.selectInBlock(math.max(start - 1, 0), math.max(end - 1, 0));
    }

    return view.setComposition(next.substring(region.start, region.end));
  }

  /// A change that ends an active composition without editing commits the
  /// preedit as it stands.
  bool _endComposition(String text, TextRange composing) {
    final state = view.editState;

    if (state?.composingStart case final int start?
        when !composing.isValid || composing.isCollapsed) {
      return view.commitComposition(
        text.substring(start + 1, state!.composingEnd! + 1),
      );
    }

    return false;
  }

  /// Replace block range [start]..[end] with [text], splitting the block at
  /// every line break.
  bool _type(int start, int end, String text) {
    final lines = text.split('\n');
    var changed = view.replaceText(start, end, lines.first);

    for (final line in lines.skip(1)) {
      changed = view.keyDown('Enter') || changed;

      if (line.isNotEmpty) {
        changed = view.insertText(line) || changed;
      }
    }

    return changed;
  }

  /// Follow the platform's selection, unless the editor's text no longer
  /// matches the platform's: an input rule changed it, and the platform
  /// gets the editor's state instead.
  bool _select(String text, TextSelection selection) {
    final state = view.editState;

    if (state == null || !selection.isValid || '$prefix${state.text}' != text) {
      return false;
    }

    final anchor = math.max(selection.baseOffset - 1, 0);
    final head = math.max(selection.extentOffset - 1, 0);

    if (state.anchor == anchor && state.head == head) {
      return false;
    }

    return view.selectInBlock(anchor, head);
  }
}
