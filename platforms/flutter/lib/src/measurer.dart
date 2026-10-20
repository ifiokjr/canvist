import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:canvist/canvist.dart';

import 'fonts.dart';

/// Measures text with `dart:ui` paragraphs, styled by [fonts] exactly as
/// the painter draws it, so layout wraps lines where Flutter's text engine
/// puts the glyphs.
final class ParagraphMeasurer implements TextMeasurer {
  const ParagraphMeasurer({this.fonts = const CanvistFonts()});

  /// How CSS font families become Flutter fonts.
  final CanvistFonts fonts;

  @override
  FontMetrics metrics(FontSpec font) {
    final paragraph = _layout(font, 'x');
    final line = paragraph.computeLineMetrics().firstOrNull;
    paragraph.dispose();

    return (ascent: line?.ascent ?? font.size, descent: line?.descent ?? 0);
  }

  /// Each grapheme cluster's advance goes to its first scalar value, and
  /// whatever rounding left over to the last, so the advances add up to
  /// the width Flutter paints the whole segment at.
  @override
  void measure(FontSpec font, String text, Float32List advances) {
    final paragraph = _layout(font, text);
    var scalar = 0;
    var offset = 0;
    var left = double.infinity;
    var right = double.negativeInfinity;
    var sum = 0.0;

    while (offset < text.length && scalar < advances.length) {
      final glyph = paragraph.getGlyphInfoAt(offset);
      final range = glyph?.graphemeClusterCodeUnitRange;
      final end = range == null || range.end <= offset ? offset + 1 : range.end;
      final bounds = glyph?.graphemeClusterLayoutBounds;

      if (bounds != null) {
        advances[scalar] = bounds.width;
        sum += bounds.width;
        left = bounds.left < left ? bounds.left : left;
        right = bounds.right > right ? bounds.right : right;
      }

      scalar += text.substring(offset, end).runes.length;
      offset = end;
    }

    if (advances.isNotEmpty && right > left) {
      advances[advances.length - 1] += (right - left) - sum;
    }

    paragraph.dispose();
  }

  ui.Paragraph _layout(FontSpec font, String text) {
    final builder = ui.ParagraphBuilder(ui.ParagraphStyle(maxLines: 1))
      ..pushStyle(fonts.style(font).getTextStyle())
      ..addText(text);

    return builder.build()
      ..layout(const ui.ParagraphConstraints(width: double.infinity));
  }
}
