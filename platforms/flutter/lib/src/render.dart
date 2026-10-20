import 'dart:math' as math;

import 'package:canvist/canvist.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

import 'painter.dart';

/// What assistive technology can ask the editor to do.
@immutable
final class EditorSemanticsActions {
  const EditorSemanticsActions({
    required this.onTap,
    required this.onCopy,
    required this.onCut,
    required this.onPaste,
    required this.onMoveByCharacter,
  });

  final VoidCallback onTap;
  final VoidCallback onCopy;
  final VoidCallback onCut;
  final VoidCallback onPaste;

  /// Move the caret one character, [forward] or back, extending the
  /// selection with [extend].
  final void Function({required bool forward, required bool extend})
  onMoveByCharacter;
}

/// The editor's viewport: lays out the part of the document that is on
/// screen, sizes the scroll extent to the document, and paints the display
/// list.
///
/// Scrolling relays out rather than repaints, because a new viewport can
/// bring blocks into view that have never been measured.
class EditorViewport extends LeafRenderObjectWidget {
  const EditorViewport({
    super.key,
    required this.view,
    required this.painter,
    required this.offset,
    required this.padding,
    required this.revision,
    required this.focused,
    required this.readOnly,
    required this.semanticsLabel,
    required this.semanticsActions,
  });

  final EditorView view;
  final DisplayPainter painter;
  final ViewportOffset offset;
  final EdgeInsets padding;

  /// Changes whenever the document, selection, or caret did, so the
  /// viewport lays out again.
  final int revision;
  final bool focused;
  final bool readOnly;
  final String? semanticsLabel;
  final EditorSemanticsActions semanticsActions;

  @override
  RenderEditorViewport createRenderObject(BuildContext context) =>
      RenderEditorViewport(
        view: view,
        painter: painter,
        offset: offset,
        padding: padding,
        focused: focused,
        readOnly: readOnly,
        semanticsLabel: semanticsLabel,
        semanticsActions: semanticsActions,
      );

  @override
  void updateRenderObject(
    BuildContext context,
    RenderEditorViewport renderObject,
  ) {
    renderObject
      ..view = view
      ..painter = painter
      ..offset = offset
      ..padding = padding
      ..focused = focused
      ..readOnly = readOnly
      ..semanticsLabel = semanticsLabel
      ..semanticsActions = semanticsActions
      ..revision = revision;
  }
}

/// The render object behind [EditorViewport].
class RenderEditorViewport extends RenderBox {
  RenderEditorViewport({
    required this._view,
    required this._painter,
    required this._offset,
    required this._padding,
    required this._focused,
    required this._readOnly,
    required this._semanticsLabel,
    required this._semanticsActions,
  });

  /// The scroll positions a frame may have to settle in before its
  /// layout and scroll extent agree.
  static const _settleRounds = 4;

  EditorView _view;
  set view(EditorView value) {
    if (!identical(value, _view)) {
      _view = value;
      _display = null;
      markNeedsLayout();
    }
  }

  DisplayPainter _painter;
  set painter(DisplayPainter value) {
    if (!identical(value, _painter)) {
      _painter = value;
      markNeedsPaint();
    }
  }

  ViewportOffset _offset;
  set offset(ViewportOffset value) {
    if (identical(value, _offset)) {
      return;
    }

    if (attached) {
      _offset.removeListener(markNeedsLayout);
      value.addListener(markNeedsLayout);
    }

    _offset = value;
    markNeedsLayout();
  }

  EdgeInsets _padding;
  set padding(EdgeInsets value) {
    if (value != _padding) {
      _padding = value;
      markNeedsLayout();
    }
  }

  bool _focused;
  set focused(bool value) {
    if (value != _focused) {
      _focused = value;
      markNeedsSemanticsUpdate();
    }
  }

  bool _readOnly;
  set readOnly(bool value) {
    if (value != _readOnly) {
      _readOnly = value;
      markNeedsSemanticsUpdate();
    }
  }

  String? _semanticsLabel;
  set semanticsLabel(String? value) {
    if (value != _semanticsLabel) {
      _semanticsLabel = value;
      markNeedsSemanticsUpdate();
    }
  }

  EditorSemanticsActions _semanticsActions;
  set semanticsActions(EditorSemanticsActions value) {
    _semanticsActions = value;
  }

  /// Lay out and paint again: the document, selection, or caret changed.
  set revision(int value) {
    markNeedsLayout();
    markNeedsSemanticsUpdate();
  }

  ViewDisplay? _display;
  double _documentHeight = 0;

  /// What the viewport paints now.
  @visibleForTesting
  ViewDisplay? get display => _display;

  /// The document's height, including estimates for blocks never laid out.
  double get documentHeight => _documentHeight;

  /// The document coordinate at the top of the viewport.
  double get documentTop => _offset.pixels - _padding.top;

  /// Where document coordinate [point] is in this box.
  Offset toLocal(Offset point) =>
      Offset(point.dx + _padding.left, point.dy - documentTop);

  /// Where local position [position] is in the document.
  Offset toDocument(Offset position) =>
      Offset(position.dx - _padding.left, position.dy + documentTop);

  @override
  void attach(PipelineOwner owner) {
    super.attach(owner);
    _offset.addListener(markNeedsLayout);
  }

  @override
  void detach() {
    _offset.removeListener(markNeedsLayout);
    super.detach();
  }

  @override
  bool get isRepaintBoundary => true;

  @override
  bool hitTestSelf(Offset position) => true;

  @override
  Size computeDryLayout(BoxConstraints constraints) =>
      constraints.biggest.isFinite
      ? constraints.biggest
      : constraints.constrain(Size(constraints.maxWidth, _documentHeight));

  @override
  void performLayout() {
    final width = constraints.maxWidth.isFinite
        ? constraints.maxWidth
        : constraints.minWidth;
    _view.setWidth(math.max(0, width - _padding.horizontal));

    if (!constraints.hasBoundedHeight) {
      _layOutWholeDocument(width);
      return;
    }

    size = constraints.biggest;

    for (var round = 0; round < _settleRounds; round++) {
      final frame = _view.frame(documentTop, size.height);
      _display = frame.display ?? _display;
      _documentHeight = frame.height;
      _offset.applyViewportDimension(size.height);
      final settled = _offset.applyContentDimensions(
        0,
        math.max(0, frame.height + _padding.vertical - size.height),
      );

      if (frame.shift != 0) {
        // Blocks above the viewport were measured: keep its content still.
        _offset.correctBy(frame.shift);
      } else if (settled) {
        break;
      }
    }
  }

  /// Without a height limit the editor is as tall as its document and
  /// never scrolls, so every block is laid out.
  void _layOutWholeDocument(double width) {
    var height = _documentHeight;

    for (var round = 0; round < _settleRounds; round++) {
      final frame = _view.frame(0, math.max(height, 1));
      _display = frame.display ?? _display;

      if (frame.height == height) {
        break;
      }

      height = frame.height;
    }

    _documentHeight = height;
    size = constraints.constrain(Size(width, height + _padding.vertical));
    _offset
      ..applyViewportDimension(size.height)
      ..applyContentDimensions(0, 0);
  }

  @override
  void paint(PaintingContext context, Offset offset) {
    final display = _display;

    if (display == null) {
      return;
    }

    final canvas = context.canvas;
    final bounds = offset & size;
    canvas
      ..save()
      ..clipRect(bounds)
      ..drawRect(bounds, Paint()..color = Color(display.background));
    _painter.paint(
      canvas,
      display,
      _view.fonts,
      origin: offset + Offset(_padding.left, _padding.top),
      top: _offset.pixels,
    );
    canvas.restore();
  }

  void _selectPlain(TextSelection selection) {
    _view.selectPlain(selection.baseOffset, selection.extentOffset);
    markNeedsLayout();
    markNeedsSemanticsUpdate();
  }

  @override
  void describeSemanticsConfiguration(SemanticsConfiguration config) {
    super.describeSemanticsConfiguration(config);
    final semantics = _view.semantics;
    final actions = _semanticsActions;

    config
      ..isSemanticBoundary = true
      ..isTextField = true
      ..isMultiline = true
      ..isReadOnly = _readOnly
      ..isFocused = _focused
      ..textDirection = TextDirection.ltr
      ..label = _semanticsLabel ?? ''
      ..value = semantics.text
      ..textSelection = TextSelection(
        baseOffset: semantics.anchor,
        extentOffset: semantics.head,
      )
      ..onTap = actions.onTap
      ..onCopy = actions.onCopy
      ..onSetSelection = _selectPlain
      ..onMoveCursorForwardByCharacter = (bool extend) {
        actions.onMoveByCharacter(forward: true, extend: extend);
      }
      ..onMoveCursorBackwardByCharacter = (bool extend) {
        actions.onMoveByCharacter(forward: false, extend: extend);
      };

    if (!_readOnly) {
      config
        ..onCut = actions.onCut
        ..onPaste = actions.onPaste;
    }
  }
}
