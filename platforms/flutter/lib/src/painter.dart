import 'dart:ui' as ui;

import 'package:canvist/canvist.dart';
import 'package:flutter/painting.dart';

import 'fonts.dart';

/// Paints canvist display lists on a [Canvas].
///
/// Text runs are shaped once and kept in a bounded cache, so a frame that
/// changes one word shapes one paragraph. The cache is per painter: keep
/// one per view, and [clear] it when the view's fonts change.
final class DisplayPainter {
  DisplayPainter({this.fonts = const CanvistFonts(), this.cacheSize = 4096});

  /// How CSS font families become Flutter fonts. It must match the
  /// measurer's, or text is drawn wider or narrower than it was laid out.
  final CanvistFonts fonts;

  /// How many shaped text runs to keep.
  final int cacheSize;

  /// Shaped runs, least recently drawn first.
  final _paragraphs = <_RunKey, ui.Paragraph>{};

  /// Paint [display] with document coordinate (0, [top]) at [origin].
  /// [fontSpecs] are the view's fonts, indexed by [ViewItem.font].
  void paint(
    Canvas canvas,
    ViewDisplay display,
    List<FontSpec> fontSpecs, {
    Offset origin = Offset.zero,
    double top = 0,
  }) {
    canvas
      ..save()
      ..translate(origin.dx, origin.dy - top);

    for (final item in display.items) {
      _item(canvas, item, fontSpecs);
    }

    canvas.restore();
  }

  /// Forget every shaped run.
  void clear() {
    for (final paragraph in _paragraphs.values) {
      paragraph.dispose();
    }

    _paragraphs.clear();
  }

  void _item(Canvas canvas, ViewItem item, List<FontSpec> fontSpecs) {
    final rect = Rect.fromLTWH(item.x, item.y, item.width, item.height);

    switch (item.kind) {
      case ViewItemKind.text:
        _text(canvas, item, fontSpecs);
      case ViewItemKind.squiggle:
        _squiggle(canvas, item);
      case ViewItemKind.checkbox:
        _checkbox(canvas, rect, checked: item.checked);
      case ViewItemKind.slot:
        _slot(canvas, rect);
      case _:
        _rect(canvas, rect, item);
    }
  }

  /// A filled rectangle: backgrounds, borders, rules, highlights, the
  /// selection, the caret, and decoration lines.
  void _rect(Canvas canvas, Rect rect, ViewItem item) {
    final paint = Paint()..color = Color(item.color);

    if (item.radius > 0) {
      canvas.drawRRect(
        RRect.fromRectAndRadius(rect, Radius.circular(item.radius)),
        paint,
      );
      return;
    }

    canvas.drawRect(rect, paint);
  }

  /// Space for a view the host draws, such as an image, outlined until one
  /// is drawn there.
  void _slot(Canvas canvas, Rect rect) {
    canvas.drawRRect(
      RRect.fromRectAndRadius(rect.deflate(0.5), const Radius.circular(4)),
      Paint()
        ..style = PaintingStyle.stroke
        ..color = const Color(0x33888888),
    );
  }

  void _text(Canvas canvas, ViewItem item, List<FontSpec> fontSpecs) {
    if (item.font >= fontSpecs.length) {
      return;
    }

    final paragraph = _paragraph(item, fontSpecs[item.font]);
    canvas.drawParagraph(
      paragraph,
      Offset(item.x, item.baseline - paragraph.alphabeticBaseline),
    );
  }

  ui.Paragraph _paragraph(ViewItem item, FontSpec font) {
    final key = (
      font: font,
      text: item.text,
      color: item.color,
      letterSpacing: item.letterSpacing,
      rtl: item.rtl,
    );
    final cached = _paragraphs.remove(key);

    if (cached != null) {
      _paragraphs[key] = cached;
      return cached;
    }

    final style = fonts.style(
      font,
      color: Color(item.color),
      letterSpacing: item.letterSpacing,
    );
    final builder =
        ui.ParagraphBuilder(
            ui.ParagraphStyle(
              maxLines: 1,
              textAlign: TextAlign.left,
              textDirection: item.rtl ? TextDirection.rtl : TextDirection.ltr,
            ),
          )
          ..pushStyle(style.getTextStyle())
          ..addText(item.text);
    final paragraph = builder.build()
      ..layout(const ui.ParagraphConstraints(width: double.infinity));

    _paragraphs[key] = paragraph;

    if (_paragraphs.length > cacheSize) {
      final oldest = _paragraphs.keys.first;
      _paragraphs.remove(oldest)!.dispose();
    }

    return paragraph;
  }

  void _squiggle(Canvas canvas, ViewItem item) {
    final thickness = item.height;
    final amplitude = thickness * 1.5;
    final wave = amplitude * 4;
    final path = Path()..moveTo(item.x, item.y);
    var position = 0.0;
    var up = true;

    while (position < item.width) {
      final step = (wave / 2).clamp(0, item.width - position).toDouble();
      path.quadraticBezierTo(
        item.x + position + step / 2,
        item.y + (up ? -amplitude : amplitude),
        item.x + position + step,
        item.y,
      );
      position += step;
      up = !up;
    }

    canvas.drawPath(
      path,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = thickness
        ..color = Color(item.color),
    );
  }

  void _checkbox(Canvas canvas, Rect rect, {required bool checked}) {
    final box = RRect.fromRectAndRadius(rect, const Radius.circular(3));
    const accent = Color(0xFF3B2CF5);

    if (checked) {
      canvas.drawRRect(box, Paint()..color = accent);
      Offset at(Offset share) =>
          rect.topLeft + Offset(rect.width * share.dx, rect.height * share.dy);
      final tick = Path()
        ..moveTo(at(_tickStart).dx, at(_tickStart).dy)
        ..lineTo(at(_tickCorner).dx, at(_tickCorner).dy)
        ..lineTo(at(_tickEnd).dx, at(_tickEnd).dy);
      canvas.drawPath(
        tick,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = rect.width * _tickWeight
          ..strokeCap = StrokeCap.round
          ..strokeJoin = StrokeJoin.round
          ..color = const Color(0xFFFFFFFF),
      );
    } else {
      canvas.drawRRect(
        box.deflate(0.5),
        Paint()
          ..style = PaintingStyle.stroke
          ..color = const Color(0xFF8A8A9A),
      );
    }
  }
}

/// A checkbox's tick, as shares of the box: where it starts, turns, and
/// ends, and how thick it is.
const _tickStart = Offset(0.22, 0.52);
const _tickCorner = Offset(0.42, 0.72);
const _tickEnd = Offset(0.78, 0.3);
const _tickWeight = 0.14;

typedef _RunKey = ({
  FontSpec font,
  String text,
  int color,
  double letterSpacing,
  bool rtl,
});
