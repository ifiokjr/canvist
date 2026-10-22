import 'dart:async';
import 'dart:math' as math;

import 'package:canvist/canvist.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';

import 'controller.dart';
import 'input.dart';
import 'painter.dart';
import 'render.dart';

/// How a [CanvistEditor] looks. Unset values follow the app: the theme's
/// brightness, and a 16 px body size scaled by the platform text scale.
@immutable
final class CanvistStyle {
  const CanvistStyle({
    this.brightness,
    this.bodyFamily,
    this.codeFamily,
    this.fontSize,
    this.lineHeight,
  });

  /// Light or dark colours.
  final Brightness? brightness;

  /// The font family list for body text and headings, as CSS writes it,
  /// such as `Inter, sans-serif`. See [CanvistFonts].
  final String? bodyFamily;

  /// The font family list for code.
  final String? codeFamily;

  /// The body font size in logical pixels, before text scaling.
  final double? fontSize;

  /// The body line height, as a multiple of the font size.
  final double? lineHeight;

  @override
  bool operator ==(Object other) =>
      other is CanvistStyle &&
      other.brightness == brightness &&
      other.bodyFamily == bodyFamily &&
      other.codeFamily == codeFamily &&
      other.fontSize == fontSize &&
      other.lineHeight == lineHeight;

  @override
  int get hashCode =>
      Object.hash(brightness, bodyFamily, codeFamily, fontSize, lineHeight);
}

/// Builds the menu a long press, a right click, or a tap on the caret
/// shows. [items] are the editor's own actions.
typedef CanvistContextMenuBuilder = Widget Function(
  BuildContext context,
  TextSelectionToolbarAnchors anchors,
  List<ContextMenuButtonItem> items,
);

/// A rich-text editor painted from canvist's Rust layout.
///
/// Text is laid out and edited in Rust and painted here from a display
/// list, measured by Flutter's own text engine, so it wraps where Flutter
/// draws it. Typing goes through the platform's text input, with IME
/// composition, autocorrect, and dictation; hardware keys run the
/// extensions' key bindings. On touch devices a long press or double tap
/// selects a word with handles and a toolbar; with a mouse, drag, double
/// click, triple click, and shift-click select.
///
/// ```dart
/// CanvistEditor(controller: controller, autofocus: true)
/// ```
class CanvistEditor extends StatefulWidget {
  const CanvistEditor({
    super.key,
    required this.controller,
    this.focusNode,
    this.autofocus = false,
    this.readOnly = false,
    this.padding = const EdgeInsets.all(16),
    this.style = const CanvistStyle(),
    this.scrollController,
    this.scrollPhysics,
    this.semanticsLabel,
    this.autocorrect = true,
    this.enableSuggestions = true,
    this.textCapitalization = TextCapitalization.sentences,
    this.keyboardAppearance,
    this.contextMenuBuilder,
  });

  /// The document and how to change it.
  final CanvistController controller;

  /// Focus for the editor; one is created when `null`.
  final FocusNode? focusNode;

  /// Whether to take focus when first built.
  final bool autofocus;

  /// Whether the document can only be read, selected, and copied.
  final bool readOnly;

  /// Space around the document, inside the scroll area.
  final EdgeInsets padding;

  /// How the document looks.
  final CanvistStyle style;

  /// Scrolls the editor; one is created when `null`.
  final ScrollController? scrollController;

  /// How the editor scrolls.
  final ScrollPhysics? scrollPhysics;

  /// What assistive technology announces the editor as.
  final String? semanticsLabel;

  /// Whether the keyboard autocorrects.
  final bool autocorrect;

  /// Whether the keyboard suggests words.
  final bool enableSuggestions;

  /// What the keyboard capitalizes.
  final TextCapitalization textCapitalization;

  /// The keyboard's brightness on iOS. Follows [style] when `null`.
  final Brightness? keyboardAppearance;

  /// Builds the context menu; the platform's adaptive toolbar when `null`.
  final CanvistContextMenuBuilder? contextMenuBuilder;

  /// Keep the caret shown instead of blinking, so tests can settle.
  static bool debugDeterministicCaret = false;

  @override
  State<CanvistEditor> createState() => _CanvistEditorState();
}

class _CanvistEditorState extends State<CanvistEditor>
    with TextInputClient, DeltaTextInputClient {
  static const _blinkInterval = Duration(milliseconds: 500);

  /// How far past the caret to scroll, so it never sits on the edge.
  static const _caretMargin = 24.0;

  final _viewportKey = GlobalKey();
  final _contextMenu = ContextMenuController();

  FocusNode? _ownFocusNode;
  FocusNode get _focusNode =>
      widget.focusNode ?? (_ownFocusNode ??= FocusNode());

  ScrollController? _ownScroll;
  ScrollController get _scroll =>
      widget.scrollController ?? (_ownScroll ??= ScrollController());

  late EditorInput _input;
  late DisplayPainter _painter;
  TextInputConnection? _connection;
  TextEditingValue _platformValue = TextEditingValue.empty;

  /// Whether the platform's composing range is one the input method marked
  /// over text the editor isn't composing, as Android keyboards mark the
  /// word at the caret for suggestions.
  var _platformMarked = false;

  /// The selection the platform had before the editor last sent it another.
  /// An input method update that returns to it without editing was made
  /// before the platform heard the editor, so it is stale.
  TextSelection? _superseded;

  ViewOptions? _options;
  Timer? _blink;
  var _caretOn = true;
  var _revision = 0;

  /// Whether selection handles show: after touch selection, until typing or
  /// a mouse takes over.
  var _handles = false;

  /// What this editor last copied: pasting the same text pastes it with
  /// its formatting, which Flutter's clipboard cannot carry.
  ViewClipboard? _copied;

  // Mouse clicks, counted for double and triple clicks.
  var _clicks = 0;
  Duration _lastClick = Duration.zero;
  Offset _lastClickPosition = Offset.zero;

  // Where a dragged handle is, in this widget's coordinates.
  Offset _handlePosition = Offset.zero;

  EditorView get _view => widget.controller.view;

  RenderEditorViewport? get _viewport =>
      _viewportKey.currentContext?.findRenderObject() as RenderEditorViewport?;

  @override
  void initState() {
    super.initState();
    _input = EditorInput(_view);
    _painter = DisplayPainter(fonts: widget.controller.fonts);
    widget.controller.addListener(_changed);
    _focusNode.addListener(_focusChanged);
    _scroll.addListener(_scrolled);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _applyStyle();
  }

  @override
  void didUpdateWidget(CanvistEditor old) {
    super.didUpdateWidget(old);

    if (!identical(old.controller, widget.controller)) {
      _swapController(old.controller);
    }

    if (old.focusNode != widget.focusNode) {
      (old.focusNode ?? _ownFocusNode)?.removeListener(_focusChanged);
      _focusNode.addListener(_focusChanged);
    }

    if (old.scrollController != widget.scrollController) {
      (old.scrollController ?? _ownScroll)?.removeListener(_scrolled);
      _scroll.addListener(_scrolled);
    }

    if (old.readOnly != widget.readOnly && _focusNode.hasFocus) {
      widget.readOnly ? _closeInput() : _openInput();
    }

    _applyStyle();
  }

  /// Show the new controller's document: its view, fonts, and focus.
  void _swapController(CanvistController old) {
    old.removeListener(_changed);
    widget.controller.addListener(_changed);
    _input = EditorInput(_view);
    _options = null;
    _painter.clear();
    _painter = DisplayPainter(fonts: widget.controller.fonts);
    _view.setFocused(_focusNode.hasFocus);
  }

  @override
  void dispose() {
    widget.controller.removeListener(_changed);
    _focusNode.removeListener(_focusChanged);
    _scroll.removeListener(_scrolled);
    _closeInput();
    _blink?.cancel();
    _contextMenu.remove();
    _painter.clear();
    _ownFocusNode?.dispose();
    _ownScroll?.dispose();
    super.dispose();
  }

  void _applyStyle() {
    final style = widget.style;
    final brightness = style.brightness ?? Theme.of(context).brightness;
    final scaler =
        MediaQuery.maybeTextScalerOf(context) ?? TextScaler.noScaling;
    final options = ViewOptions(
      dark: brightness == Brightness.dark,
      bodyFamily: style.bodyFamily,
      codeFamily: style.codeFamily,
      size: scaler.scale(style.fontSize ?? 16),
      lineHeight: style.lineHeight,
      overscan: 400,
    );

    if (options != _options) {
      _options = options;
      _view.setOptions(options);
      _revision++;
    }
  }

  /// The document or selection changed: lay out again and tell the
  /// platform's text input.
  void _changed() {
    if (!mounted) {
      return;
    }

    setState(() => _revision++);
    _restartBlink();

    if (_connection?.attached ?? false) {
      _sendValue();
    }
  }

  void _scrolled() {
    if (_handles) {
      setState(() {});
    }

    _contextMenu.remove();
  }

  void _focusChanged() {
    final focused = _focusNode.hasFocus;
    _view.setFocused(focused);

    if (focused) {
      _openInput();
      _restartBlink();
    } else {
      _closeInput();
      _blink?.cancel();
      _handles = false;
      _contextMenu.remove();
    }

    setState(() => _revision++);
  }

  // Caret blink.
  void _restartBlink() {
    _blink?.cancel();
    _setCaret(on: true);

    if (!_focusNode.hasFocus || CanvistEditor.debugDeterministicCaret) {
      return;
    }

    _blink = Timer.periodic(_blinkInterval, (_) => _setCaret(on: !_caretOn));
  }

  void _setCaret({required bool on}) {
    if (_caretOn == on || !mounted) {
      return;
    }

    _caretOn = on;
    _view.setCaretOn(on);
    setState(() => _revision++);
  }

  // Text input.
  void _openInput() {
    if (widget.readOnly) {
      return;
    }

    final connection = _connection;

    if (connection != null && connection.attached) {
      connection.show();
      return;
    }

    final brightness =
        widget.keyboardAppearance ??
        (_options?.dark ?? false ? Brightness.dark : Brightness.light);
    _connection = TextInput.attach(
      this,
      TextInputConfiguration(
        viewId: View.of(context).viewId,
        inputType: TextInputType.multiline,
        inputAction: TextInputAction.newline,
        enableDeltaModel: true,
        autocorrect: widget.autocorrect,
        enableSuggestions: widget.enableSuggestions,
        textCapitalization: widget.textCapitalization,
        keyboardAppearance: brightness,
      ),
    );
    _platformValue = _input.value;
    _platformMarked = false;
    _superseded = null;
    _connection!
      ..setEditingState(_platformValue)
      ..show();
    _updateInputGeometry();
  }

  void _closeInput() {
    _connection?.close();
    _connection = null;
  }

  /// Give the platform the editor's state when it differs from what the
  /// platform has.
  ///
  /// A range the input method marked stays marked: sending the editor's
  /// value would clear it, and Android restarts the input method, which
  /// marks the word again, without end.
  void _sendValue() {
    final value = _input.value;
    final marked =
        _platformMarked &&
        value.text == _platformValue.text &&
        value.selection == _platformValue.selection;

    if (value != _platformValue && !marked) {
      _superseded = value.selection == _platformValue.selection
          ? null
          : _platformValue.selection;
      _platformValue = value;
      _platformMarked = false;
      _connection?.setEditingState(value);
    }

    _updateInputGeometry();
  }

  /// Whether [value], from the platform, only returns to the selection the
  /// editor has since replaced: the input method made it before the
  /// platform heard the editor, which will overwrite it.
  bool _stale(TextEditingValue value) =>
      value.text == _platformValue.text && value.selection == _superseded;

  /// Whether the platform's composing range, after its latest edit, marks
  /// text the editor isn't composing.
  bool get _markedByPlatform =>
      !_input.composing && !_platformValue.composing.isCollapsed;

  /// Tell the platform where the editor and its caret are, for IME
  /// candidate windows and the iOS floating cursor.
  void _updateInputGeometry() {
    SchedulerBinding.instance.addPostFrameCallback((_) {
      final connection = _connection;
      final viewport = _viewport;

      if (!mounted ||
          connection == null ||
          !connection.attached ||
          viewport == null ||
          !viewport.hasSize) {
        return;
      }

      connection.setEditableSizeAndTransform(
        viewport.size,
        viewport.getTransformTo(null),
      );

      if (_view.compositionBounds case final bounds?) {
        connection.setComposingRect(_local(viewport, bounds));
      }

      if (_view.caretRect case final caret?) {
        connection.setCaretRect(_local(viewport, caret));
      }
    }, debugLabel: 'CanvistEditor.inputGeometry');
  }

  @override
  TextEditingValue get currentTextEditingValue => _platformValue;

  @override
  AutofillScope? get currentAutofillScope => null;

  @override
  void updateEditingValueWithDeltas(List<TextEditingDelta> deltas) {
    final expected = deltas.fold(
      _platformValue,
      (value, delta) => delta.apply(value),
    );

    if (_stale(expected)) {
      return;
    }

    _platformValue = expected;
    _superseded = null;

    if (widget.readOnly) {
      return;
    }

    final changed = _input.applyDeltas(deltas);
    _platformMarked = _markedByPlatform;
    _userEdited(changed);
  }

  @override
  void updateEditingValue(TextEditingValue value) {
    final previous = _platformValue;

    if (_stale(value)) {
      return;
    }

    _platformValue = value;
    _superseded = null;

    if (widget.readOnly || value == previous) {
      return;
    }

    final changed = _input.applyValue(previous, value);
    _platformMarked = _markedByPlatform;
    _userEdited(changed);
  }

  /// The user changed the document or selection through input.
  void _userEdited(bool changed) {
    if (changed) {
      _handles = false;
      _contextMenu.remove();
      widget.controller.refresh();
      _showCaret();
    } else {
      _sendValue();
    }
  }

  @override
  void performAction(TextInputAction action) {
    // A line break arrives as typed text, which splits the block; nothing
    // else applies to a multiline editor.
  }

  @override
  void performPrivateCommand(String action, Map<String, dynamic> data) {}

  @override
  void updateFloatingCursor(RawFloatingCursorPoint point) {}

  @override
  void showAutocorrectionPromptRect(int start, int end) {}

  @override
  void connectionClosed() {
    _connection = null;
    _focusNode.unfocus();
  }

  @override
  void showToolbar() => _showMenu();

  // Keys.
  KeyEventResult _key(FocusNode node, KeyEvent event) {
    if (event is KeyUpEvent || _input.composing) {
      // Keys belong to the input method while it composes.
      return KeyEventResult.ignored;
    }

    final keyboard = HardwareKeyboard.instance;
    final shift = keyboard.isShiftPressed;
    final ctrl = keyboard.isControlPressed;
    final alt = keyboard.isAltPressed;
    final meta = keyboard.isMetaPressed;
    // The engine's platform decides what `Mod` is in its key bindings, so it
    // decides the clipboard shortcuts too.
    final primary = widget.controller.engine.platform == KeyPlatform.apple
        ? meta && !ctrl
        : ctrl && !meta;
    final name = _domKey(event.logicalKey);

    if (name == null) {
      return KeyEventResult.ignored;
    }

    final printable = name.length == 1;

    if (primary && !alt && !shift && printable) {
      switch (name) {
        case 'c':
          _copy();
          return KeyEventResult.handled;
        case 'x':
          _cut();
          return KeyEventResult.handled;
        case 'v':
          unawaited(_paste());
          return KeyEventResult.handled;
      }
    }

    // Printable keys without a command modifier are text, which arrives
    // through the platform's text input.
    if (printable && !ctrl && !meta) {
      return KeyEventResult.ignored;
    }

    if (widget.readOnly &&
        !_navigation.contains(name) &&
        !(primary && name == 'a')) {
      return KeyEventResult.ignored;
    }

    final handled = _view.keyDown(
      name,
      shift: shift,
      ctrl: ctrl,
      alt: alt,
      meta: meta,
    );

    if (!handled) {
      return KeyEventResult.ignored;
    }

    _handles = false;
    _contextMenu.remove();
    widget.controller.refresh();
    _showCaret();
    return KeyEventResult.handled;
  }

  bool get _isApple => switch (defaultTargetPlatform) {
    TargetPlatform.iOS || TargetPlatform.macOS => true,
    _ => false,
  };

  static const _navigation = {
    'ArrowLeft',
    'ArrowRight',
    'ArrowUp',
    'ArrowDown',
    'Home',
    'End',
    'PageUp',
    'PageDown',
  };

  /// The DOM `KeyboardEvent.key` name canvist key bindings use.
  static String? _domKey(LogicalKeyboardKey key) {
    final named = _domNames[key];

    if (named != null) {
      return named;
    }

    final label = key.keyLabel;

    return label.runes.length == 1 ? label.toLowerCase() : null;
  }

  static final _domNames = {
    LogicalKeyboardKey.arrowLeft: 'ArrowLeft',
    LogicalKeyboardKey.arrowRight: 'ArrowRight',
    LogicalKeyboardKey.arrowUp: 'ArrowUp',
    LogicalKeyboardKey.arrowDown: 'ArrowDown',
    LogicalKeyboardKey.backspace: 'Backspace',
    LogicalKeyboardKey.delete: 'Delete',
    LogicalKeyboardKey.enter: 'Enter',
    LogicalKeyboardKey.numpadEnter: 'Enter',
    LogicalKeyboardKey.tab: 'Tab',
    LogicalKeyboardKey.home: 'Home',
    LogicalKeyboardKey.end: 'End',
    LogicalKeyboardKey.pageUp: 'PageUp',
    LogicalKeyboardKey.pageDown: 'PageDown',
    LogicalKeyboardKey.escape: 'Escape',
    LogicalKeyboardKey.space: ' ',
  };

  // Clipboard.
  void _copy() {
    final copied = _view.copySelection();

    if (copied != null) {
      _copied = copied;
      unawaited(Clipboard.setData(ClipboardData(text: copied.text)));
    }
  }

  void _cut() {
    if (widget.readOnly) {
      return _copy();
    }

    final cut = _view.cutSelection();

    if (cut != null) {
      _copied = cut;
      unawaited(Clipboard.setData(ClipboardData(text: cut.text)));
      _userEdited(true);
    }
  }

  Future<void> _paste() async {
    if (widget.readOnly) {
      return;
    }

    final text = (await Clipboard.getData(Clipboard.kTextPlain))?.text;

    if (!mounted || text == null || text.isEmpty) {
      return;
    }

    final copied = _copied;
    _userEdited(
      copied != null && copied.text == text
          ? _view.pasteHtml(copied.html)
          : _view.pasteText(text),
    );
  }

  void _selectAll() {
    widget.controller.selectAll();
    _showMenu();
  }

  // Pointers.
  Offset? _document(Offset local) {
    final viewport = _viewport;
    return viewport?.toDocument(local);
  }

  void _focus() {
    if (_focusNode.hasFocus) {
      _openInput();
    } else {
      _focusNode.requestFocus();
    }
  }

  /// A mouse or trackpad press: a right click opens the menu, keeping a
  /// selection it lands in; other clicks place the caret, extend with
  /// Shift, and count up to triple clicks.
  void _mouseDown(PointerDownEvent event) {
    final kind = event.kind;

    if (kind != PointerDeviceKind.mouse && kind != PointerDeviceKind.trackpad) {
      return;
    }

    _focus();
    _handles = false;
    _contextMenu.remove();
    final point = _document(event.localPosition);

    if (point == null) {
      return;
    }

    if (event.buttons == kSecondaryMouseButton) {
      _secondaryClick(point, event.position);
      return;
    }

    final clicks = _countClick(event);
    final extend = HardwareKeyboard.instance.isShiftPressed;
    final changed = _view.pointerDown(
      point.dx,
      point.dy,
      extend: extend,
      clicks: clicks,
    );

    if (changed) {
      widget.controller.refresh();
    }
  }

  /// Open the menu at [position], first moving the caret to [point] unless
  /// it is inside the selection.
  void _secondaryClick(Offset point, Offset position) {
    if (!_view.isInSelection(point.dx, point.dy)) {
      _view.pointerDown(point.dx, point.dy);
      _view.pointerUp();
      widget.controller.refresh();
    }

    _showMenu(at: position);
  }

  /// One click more than the last if [event] follows it closely in time
  /// and place, up to three; otherwise one.
  int _countClick(PointerDownEvent event) {
    final soon = event.timeStamp - _lastClick <= kDoubleTapTimeout;
    final near =
        (event.position - _lastClickPosition).distance <= kDoubleTapSlop;
    _clicks = soon && near ? math.min(_clicks + 1, 3) : 1;
    _lastClick = event.timeStamp;
    _lastClickPosition = event.position;

    return _clicks;
  }

  void _mouseMove(PointerMoveEvent event) {
    if (event.kind != PointerDeviceKind.mouse ||
        event.buttons != kPrimaryMouseButton) {
      return;
    }

    final point = _document(event.localPosition);

    if (point != null && _view.pointerMove(point.dx, point.dy)) {
      widget.controller.refresh();
      _showCaret();
    }
  }

  void _mouseUp(PointerUpEvent event) {
    if (event.kind == PointerDeviceKind.mouse) {
      _view.pointerUp();
    }
  }

  void _tap(TapUpDetails details) {
    final point = _document(details.localPosition);
    _focus();

    if (point == null) {
      return;
    }

    final wasCaret = _view.caretRect;
    _view.pointerDown(point.dx, point.dy);
    _view.pointerUp();
    final caret = _view.caretRect;
    // A second tap on the caret shows its handle and the menu, as text
    // fields do.
    final again = wasCaret != null && caret != null && wasCaret == caret;
    _handles = again;
    again ? _showMenu() : _contextMenu.remove();
    widget.controller.refresh();
  }

  void _selectWord(Offset local) {
    final point = _document(local);
    _focus();

    if (point == null) {
      return;
    }

    _view.pointerDown(point.dx, point.dy, clicks: 2);
    _handles = true;
    _contextMenu.remove();
    widget.controller.refresh();
  }

  void _longPressMove(LongPressMoveUpdateDetails details) {
    final point = _document(details.localPosition);

    if (point != null && _view.pointerMove(point.dx, point.dy)) {
      widget.controller.refresh();
    }
  }

  void _longPressEnd(LongPressEndDetails details) {
    _view.pointerUp();
    _showMenu();
  }

  // Selection handles.
  void _handleStart(Offset endpoint) => _handlePosition = endpoint;

  void _handleUpdate(
    DragUpdateDetails details, {
    required bool start,
    required double lineHeight,
  }) {
    _handlePosition += details.delta;
    // The handle hangs below the line: aim for the middle of the line.
    final point = _document(_handlePosition - Offset(0, lineHeight / 2));

    if (point == null) {
      return;
    }

    final moved = _view.selectionEnds?.collapsed ?? true
        ? _view.pointerDown(point.dx, point.dy)
        : _view.moveSelectionEnd(point.dx, point.dy, start: start);
    _view.pointerUp();

    if (moved) {
      _contextMenu.remove();
      widget.controller.refresh();
    }
  }

  List<Widget> _handleWidgets(BuildContext context) {
    final viewport = _viewport;
    final ends = _view.selectionEnds;

    if (!_handles || viewport == null || ends == null || !_focusNode.hasFocus) {
      return const [];
    }

    final controls = _isApple
        ? cupertinoTextSelectionHandleControls
        : materialTextSelectionHandleControls;
    Widget handle(
      ViewRect caret,
      TextSelectionHandleType type, {
      required bool start,
    }) {
      final lineHeight = caret.height;
      final bottom = _local(viewport, caret).bottomLeft;
      final size = controls.getHandleSize(lineHeight);
      final anchor = controls.getHandleAnchor(type, lineHeight);

      return Positioned(
        left: bottom.dx - anchor.dx,
        top: bottom.dy - anchor.dy,
        width: size.width,
        height: size.height,
        child: GestureDetector(
          behavior: HitTestBehavior.translucent,
          dragStartBehavior: DragStartBehavior.down,
          onPanStart: (_) => _handleStart(bottom),
          onPanUpdate: (details) =>
              _handleUpdate(details, start: start, lineHeight: lineHeight),
          onPanEnd: (_) => _showMenu(),
          child: controls.buildHandle(context, type, lineHeight),
        ),
      );
    }

    if (ends.collapsed) {
      return [
        handle(ends.end, TextSelectionHandleType.collapsed, start: false),
      ];
    }

    return [
      handle(ends.start, TextSelectionHandleType.left, start: true),
      handle(ends.end, TextSelectionHandleType.right, start: false),
    ];
  }

  // Context menu.
  /// Show the context menu over the selection, or at [at] for a right
  /// click.
  void _showMenu({Offset? at}) {
    final viewport = _viewport;

    if (!mounted || viewport == null) {
      return;
    }

    final anchors = _menuAnchors(viewport, at);
    final items = _menuItems();

    _contextMenu.show(
      context: context,
      contextMenuBuilder: (context) =>
          widget.contextMenuBuilder?.call(context, anchors, items) ??
          AdaptiveTextSelectionToolbar.buttonItems(
            anchors: anchors,
            buttonItems: items,
          ),
    );
  }

  /// Where the menu goes: at a click, or above the selection with room
  /// below it to flip to.
  TextSelectionToolbarAnchors _menuAnchors(
    RenderEditorViewport viewport,
    Offset? at,
  ) {
    final origin = viewport.localToGlobal(Offset.zero);

    if (at != null) {
      return TextSelectionToolbarAnchors(primaryAnchor: at);
    }

    final bounds = _view.selectionBounds ?? _view.caretRect;

    if (bounds == null) {
      return TextSelectionToolbarAnchors(primaryAnchor: origin);
    }

    final selection = _local(viewport, bounds).shift(origin);

    return TextSelectionToolbarAnchors(
      primaryAnchor: selection.topCenter,
      secondaryAnchor: selection.bottomCenter,
    );
  }

  /// Cut, copy, paste, and select all, as far as they apply.
  List<ContextMenuButtonItem> _menuItems() {
    final selected = _view.copySelection() != null;
    final editable = !widget.readOnly;
    ContextMenuButtonItem item(
      ContextMenuButtonType type,
      FutureOr<void> Function() action,
    ) => ContextMenuButtonItem(type: type, onPressed: () => _menu(action));

    return [
      if (selected && editable) item(ContextMenuButtonType.cut, _cut),
      if (selected) item(ContextMenuButtonType.copy, _copy),
      if (editable) item(ContextMenuButtonType.paste, _paste),
      ContextMenuButtonItem(
        type: ContextMenuButtonType.selectAll,
        onPressed: _selectAll,
      ),
    ];
  }

  void _menu(FutureOr<void> Function() action) {
    _contextMenu.remove();
    unawaited(Future.sync(action));
  }

  // Scrolling.
  /// Scroll the caret, or the moving end of the selection, into view after
  /// this frame lays out.
  void _showCaret() {
    SchedulerBinding.instance.addPostFrameCallback((_) {
      final viewport = _viewport;

      if (!mounted || viewport == null || !_scroll.hasClients) {
        return;
      }

      final rect = _view.caretRect ?? _view.selectionEnds?.end;

      if (rect == null) {
        return;
      }

      final position = _scroll.position;
      final top = rect.y + widget.padding.top;
      final bottom = top + rect.height;
      final target = top - _caretMargin < position.pixels
          ? top - _caretMargin
          : bottom + _caretMargin > position.pixels + position.viewportDimension
          ? bottom + _caretMargin - position.viewportDimension
          : null;

      if (target != null) {
        _scroll.jumpTo(
          target.clamp(
            position.minScrollExtent,
            math.max(position.maxScrollExtent, target),
          ),
        );
      }

      _updateInputGeometry();
    }, debugLabel: 'CanvistEditor.showCaret');
  }

  Rect _local(RenderEditorViewport viewport, ViewRect rect) =>
      viewport.toLocal(Offset(rect.x, rect.y)) & Size(rect.width, rect.height);

  /// Assistive technology and UI automation replace the whole field's text.
  /// Apply only the part that changed, so the formatting around it
  /// survives: a line break in the new text splits the block, as Enter does.
  void _setText(String text) {
    if (widget.readOnly) {
      return;
    }

    final delta = EditorInput.delta(
      TextEditingValue(text: _view.semantics.text),
      TextEditingValue(text: text),
    );

    if (delta is! TextEditingDeltaReplacement) {
      return;
    }

    final range = delta.replacedRange;
    _view.selectPlain(range.start, range.end);
    final lines = delta.replacementText.split('\n');

    if (lines.first.isNotEmpty) {
      _view.insertText(lines.first);
    } else if (!range.isCollapsed) {
      _view.keyDown('Delete');
    }

    for (final line in lines.skip(1)) {
      _view.keyDown('Enter');

      if (line.isNotEmpty) {
        _view.insertText(line);
      }
    }

    _userEdited(true);
  }

  void _moveByCharacter({required bool forward, required bool extend}) {
    if (_view.keyDown(forward ? 'ArrowRight' : 'ArrowLeft', shift: extend)) {
      widget.controller.refresh();
    }
  }

  @override
  Widget build(BuildContext context) {
    final editor = Scrollable(
      controller: _scroll,
      physics: widget.scrollPhysics,
      viewportBuilder: (context, offset) => EditorViewport(
        key: _viewportKey,
        view: _view,
        painter: _painter,
        offset: offset,
        padding: widget.padding,
        revision: _revision,
        focused: _focusNode.hasFocus,
        readOnly: widget.readOnly,
        semanticsLabel: widget.semanticsLabel,
        semanticsActions: EditorSemanticsActions(
          onTap: _focus,
          onCopy: _copy,
          onCut: _cut,
          onPaste: () => unawaited(_paste()),
          onMoveByCharacter: _moveByCharacter,
          onSetText: _setText,
        ),
      ),
    );

    return Focus(
      focusNode: _focusNode,
      autofocus: widget.autofocus,
      onKeyEvent: _key,
      child: MouseRegion(
        cursor: SystemMouseCursors.text,
        child: Listener(
          onPointerDown: _mouseDown,
          onPointerMove: _mouseMove,
          onPointerUp: _mouseUp,
          child: GestureDetector(
            supportedDevices: const {
              PointerDeviceKind.touch,
              PointerDeviceKind.stylus,
              PointerDeviceKind.invertedStylus,
              PointerDeviceKind.unknown,
            },
            onTapUp: _tap,
            onDoubleTapDown: (details) {
              _selectWord(details.localPosition);
              _showMenu();
            },
            onLongPressStart: (details) => _selectWord(details.localPosition),
            onLongPressMoveUpdate: _longPressMove,
            onLongPressEnd: _longPressEnd,
            child: Stack(
              clipBehavior: Clip.none,
              children: [editor, ..._handleWidgets(context)],
            ),
          ),
        ),
      ),
    );
  }
}
