part of 'engine.dart';

/// A font that layout asks a [TextMeasurer] to measure.
final class FontSpec {
  const FontSpec({
    required this.family,
    required this.size,
    required this.weight,
    required this.italic,
  });

  /// The CSS `font-family` list, such as `Inter, ui-sans-serif, sans-serif`.
  final String family;

  /// The size in logical pixels.
  final double size;

  /// The CSS weight, 100 to 900.
  final int weight;

  /// Whether the face is italic.
  final bool italic;

  /// The names in [family], unquoted, in fallback order.
  List<String> get families => [
    for (final name in family.split(','))
      if (name.trim().replaceAll(RegExp('^["\']|["\']\$'), '') case final name
          when name.isNotEmpty)
        name,
  ];

  @override
  bool operator ==(Object other) =>
      other is FontSpec &&
      other.family == family &&
      other.size == size &&
      other.weight == weight &&
      other.italic == italic;

  @override
  int get hashCode => Object.hash(family, size, weight, italic);

  @override
  String toString() =>
      'FontSpec(${italic ? 'italic ' : ''}$weight ${size}px $family)';
}

/// A font's distance above and below the baseline, in logical pixels, both
/// positive.
typedef FontMetrics = ({double ascent, double descent});

/// Measures text the way the host paints it. Flutter measures with
/// `dart:ui` paragraphs; [FixedMeasurer] is a stand-in for tests and
/// servers.
///
/// Layout caches every answer by font and text for the life of a view, so
/// each is asked once.
abstract interface class TextMeasurer {
  /// The vertical metrics of [font].
  FontMetrics metrics(FontSpec font);

  /// Write the advance of each Unicode scalar value of [text] in [font]
  /// into [advances], which has one slot per scalar value. They must sum to
  /// the width the host paints [text] at, kerning and ligatures included;
  /// how a ligature's width is split among its characters is up to the
  /// host.
  void measure(FontSpec font, String text, Float32List advances);
}

/// Every character [advance] wide, with an ascent of 80% and a descent of
/// 20% of the font size: predictable layout for tests and servers.
final class FixedMeasurer implements TextMeasurer {
  const FixedMeasurer({this.advance = 0.6});

  /// Each character's advance as a share of the font size.
  final double advance;

  @override
  FontMetrics metrics(FontSpec font) =>
      (ascent: font.size * 0.8, descent: font.size * 0.2);

  @override
  void measure(FontSpec font, String text, Float32List advances) =>
      advances.fillRange(0, advances.length, font.size * advance);
}

/// One measurement batch, as `platforms/dart/rust/src/measure.rs` lays it
/// out.
final class _MeasureRequest extends Struct {
  @Uint32()
  external int firstFont;

  external Pointer<Uint8> fontsJson;

  @Size()
  external int fontsJsonLength;

  external Pointer<Uint32> metrics;

  @Size()
  external int metricsLength;

  external Pointer<Uint32> segmentFonts;

  external Pointer<Uint32> segmentLengths;

  @Size()
  external int segmentsLength;

  external Pointer<Uint16> text;

  @Size()
  external int textLength;

  external Pointer<Float> out;

  @Size()
  external int outLength;
}

typedef _MeasureNative = Void Function(Pointer<_MeasureRequest>);

/// A document laid out for a UI host: what to paint, where things are, and
/// the input the host forwards. Create one with [Doc.view].
///
/// The view shares its document, so commands run on the [Doc], and
/// collaborators' updates applied to it, reach the layout on the next call.
/// Every call that changes something goes through the editor with the wall
/// clock and this view's layout, so typing groups into undo steps by time
/// and arrow keys move by visual lines.
///
/// Coordinates are logical pixels from the top-left of the document.
/// Offsets in [ViewEditState] and [ViewSemantics] count UTF-16 code units,
/// like Dart strings.
final class EditorView {
  EditorView._(this.doc, this._measurer, ViewOptions options) {
    _callable = NativeCallable<_MeasureNative>.isolateLocal(_measure);
    _bridge = _guard(
      () => doc._bridge.view(
        measurer: BigInt.from(_callable.nativeFunction.address),
        options: options,
      ),
    );
  }

  /// The document this view shows.
  final Doc doc;

  final TextMeasurer _measurer;
  late final NativeCallable<_MeasureNative> _callable;
  late final CanvistView _bridge;
  final _fonts = <FontSpec>[];
  Object? _measureError;
  StackTrace? _measureTrace;

  /// The fonts layout has used, indexed by [ViewItem.font].
  List<FontSpec> get fonts => List.unmodifiable(_fonts);

  /// Lay the document out [width] logical pixels wide.
  void setWidth(double width) => _call(() => _bridge.setWidth(width: width));

  /// Change how the document looks. Measurements are kept.
  void setOptions(ViewOptions options) =>
      _call(() => _bridge.setOptions(options: options));

  /// Forget every measurement, as when a font loads or the text scale
  /// changes.
  void forgetMeasurements() {
    _fonts.clear();
    _call(_bridge.forgetMeasurements);
  }

  /// Show the caret while the host has focus.
  void setFocused(bool focused) =>
      _call(() => _bridge.setFocused(focused: focused));

  /// Show or hide the caret, to blink it.
  void setCaretOn(bool on) => _call(() => _bridge.setCaretOn(on_: on));

  /// Lay out the viewport from [top] down [height] logical pixels. The
  /// frame's display is `null` when it is what the last frame returned.
  ViewFrame frame(double top, double height) =>
      _call(() => _bridge.frame(top: top, height: height));

  /// Run the key binding for [key], a DOM `KeyboardEvent.key` name such as
  /// `Enter`, `ArrowLeft`, `Backspace`, or `b`. Returns whether one applied.
  bool keyDown(
    String key, {
    bool shift = false,
    bool ctrl = false,
    bool alt = false,
    bool meta = false,
  }) => _call(
    () => _bridge.keyDown(
      key: key,
      shift: shift,
      ctrl: ctrl,
      alt: alt,
      meta: meta,
    ),
  );

  /// Replace the selection with [text] as typing does: input rules apply.
  bool insertText(String text) => _call(() => _bridge.insertText(text: text));

  /// The text block a platform input method edits, or `null` when the
  /// selection's head is not in a text block.
  ViewEditState? get editState => _call(_bridge.editState);

  /// Select [anchor]..[head] of [editState]'s text.
  bool selectInBlock(int anchor, int head) =>
      _call(() => _bridge.selectInBlock(anchor: anchor, head: head));

  /// Replace [start]..[end] of [editState]'s text with [text]. A
  /// replacement at a caret is typing, so input rules apply.
  bool replaceText(int start, int end, String text) =>
      _call(() => _bridge.replaceText(start: start, end: end, text: text));

  /// Show [text] as IME preedit text over the selection or the previous
  /// preedit.
  bool setComposition(String text) =>
      _call(() => _bridge.setComposition(text: text));

  /// Commit the composition as [text].
  bool commitComposition(String text) =>
      _call(() => _bridge.commitComposition(text: text));

  /// Remove the preedit and end the composition.
  bool cancelComposition() => _call(_bridge.cancelComposition);

  /// Press the pointer at ([x], [y]): one click places the caret, or
  /// extends the selection with [extend]; two select a word and three the
  /// whole text block.
  bool pointerDown(double x, double y, {bool extend = false, int clicks = 1}) =>
      _call(
        () => _bridge.pointerDown(x: x, y: y, extend: extend, clicks: clicks),
      );

  /// Drag a pressed pointer to ([x], [y]), extending the selection.
  bool pointerMove(double x, double y) =>
      _call(() => _bridge.pointerMove(x: x, y: y));

  /// Release the pointer.
  void pointerUp() => _call(_bridge.pointerUp);

  /// Whether ([x], [y]) is inside a non-empty selection.
  bool isInSelection(double x, double y) =>
      _call(() => _bridge.isInSelection(x: x, y: y));

  /// Move the selection's start (or, without [start], its end) to ([x],
  /// [y]), as selection handles do.
  bool moveSelectionEnd(double x, double y, {required bool start}) =>
      _call(() => _bridge.moveSelectionEnd(start: start, x: x, y: y));

  /// The selection as HTML and plain text, or `null` when it is empty.
  ViewClipboard? copySelection() => _call(_bridge.copySelection);

  /// Cut the selection, returning what [copySelection] would.
  ViewClipboard? cutSelection() => _call(_bridge.cutSelection);

  /// Paste plain text over the selection; paste rules apply.
  bool pasteText(String text) => _call(() => _bridge.pasteText(text: text));

  /// Paste HTML over the selection.
  bool pasteHtml(String html) => _call(() => _bridge.pasteHtml(html: html));

  /// Run [command] from one of the engine's handles.
  bool run(Command command) {
    doc._check(command._registration, command.name);

    return runNamed(command.name, command.args);
  }

  /// Run the command named [name].
  bool runNamed(String name, [Map<String, Object?> args = const {}]) =>
      _call(() => _bridge.run(name: name, args: jsonEncode(args)));

  /// Undo the last change.
  bool undo() => _call(_bridge.undo);

  /// Redo the last undone change.
  bool redo() => _call(_bridge.redo);

  /// The caret's rectangle, or `null` when the selection is not a caret.
  ViewRect? get caretRect => _call(_bridge.caretRect);

  /// The caret rectangles at the selection's start and end.
  ViewSelectionEnds? get selectionEnds => _call(_bridge.selectionEnds);

  /// The smallest rectangle around the selection.
  ViewRect? get selectionBounds => _call(_bridge.selectionBounds);

  /// The smallest rectangle around the IME composition.
  ViewRect? get compositionBounds => _call(_bridge.compositionBounds);

  /// The document's plain text and selection, for accessibility.
  ViewSemantics get semantics => _call(_bridge.semantics);

  /// Select [anchor]..[head] of [semantics]' text.
  bool selectPlain(int anchor, int head) =>
      _call(() => _bridge.selectPlain(anchor: anchor, head: head));

  /// Release the view. The document stays usable.
  void dispose() {
    _bridge.dispose();
    _callable.close();
  }

  /// Run [call], raising canvist failures as [CanvistException]s and
  /// anything the measurer threw while Rust waited on it.
  T _call<T>(T Function() call) {
    final result = _guard(call);
    final error = _measureError;
    final trace = _measureTrace;

    if (error != null) {
      _measureError = null;
      _measureTrace = null;
      Error.throwWithStackTrace(error, trace ?? StackTrace.current);
    }

    return result;
  }

  /// Answer one measurement batch from Rust. It runs inside a view call, so
  /// it must not call back into the view.
  void _measure(Pointer<_MeasureRequest> pointer) {
    try {
      _answer(pointer.ref);
    } on Object catch (error, trace) {
      _measureError ??= error;
      _measureTrace ??= trace;
    }
  }

  void _answer(_MeasureRequest request) {
    if (request.fontsJsonLength > 0) {
      final json = utf8.decode(
        request.fontsJson.asTypedList(request.fontsJsonLength),
      );
      _fonts.length = request.firstFont;

      for (final font in jsonDecode(json) as List<Object?>) {
        final fields = font! as Map<String, Object?>;
        _fonts.add(
          FontSpec(
            family: fields['family']! as String,
            size: (fields['size']! as num).toDouble(),
            weight: fields['weight']! as int,
            italic: fields['italic']! as bool,
          ),
        );
      }
    }

    final out = request.out.asTypedList(request.outLength);
    final metrics = request.metrics.asTypedList(request.metricsLength);

    for (final (index, font) in metrics.indexed) {
      final (:ascent, :descent) = _measurer.metrics(_fonts[font]);
      out[index * 2] = ascent;
      out[index * 2 + 1] = descent;
    }

    final text = String.fromCharCodes(
      request.text.asTypedList(request.textLength),
    );
    final fonts = request.segmentFonts.asTypedList(request.segmentsLength);
    final lengths = request.segmentLengths.asTypedList(request.segmentsLength);
    var start = 0;
    var slot = metrics.length * 2;

    for (final (index, length) in lengths.indexed) {
      final segment = text.substring(start, start + length);
      final scalars = segment.runes.length;
      _measurer.measure(
        _fonts[fonts[index]],
        segment,
        Float32List.sublistView(out, slot, slot + scalars),
      );
      start += length;
      slot += scalars;
    }
  }
}
