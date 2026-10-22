/// The painter draws ink strokes as round-capped polylines, so a single
/// point still leaves a dot.
library;

import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

const _size = 20;

ViewItem _stroke(List<double> points, {required double width}) => ViewItem(
  kind: ViewItemKind.stroke,
  x: 0,
  y: 0,
  width: 0,
  height: 0,
  color: 0xFF000000,
  radius: 0,
  text: '',
  font: 0,
  baseline: 0,
  letterSpacing: 0,
  rtl: false,
  checked: false,
  points: Float32List.fromList(points),
  lineWidth: width,
);

/// The RGBA pixels of [items] painted on a transparent square.
Future<Uint8List> _paint(List<ViewItem> items) async {
  final recorder = ui.PictureRecorder();
  final display = ViewDisplay(
    fonts: const [],
    items: items,
    background: 0xFFFFFFFF,
  );
  DisplayPainter().paint(ui.Canvas(recorder), display, const []);
  final image = await recorder.endRecording().toImage(_size, _size);
  final bytes = await image.toByteData();
  image.dispose();
  return bytes!.buffer.asUint8List();
}

int _alpha(Uint8List pixels, int x, int y) => pixels[(y * _size + x) * 4 + 3];

void main() {
  test('a stroke paints along its points and nowhere else', () async {
    final pixels = await _paint([
      _stroke([2, 10, 18, 10], width: 4),
    ]);

    expect(_alpha(pixels, 10, 10), 255);
    expect(_alpha(pixels, 1, 10), greaterThan(0), reason: 'the cap is round');
    expect(_alpha(pixels, 10, 3), 0);
  });

  test('a stroke of one point paints a dot', () async {
    final pixels = await _paint([
      _stroke([10, 10], width: 6),
    ]);

    expect(_alpha(pixels, 10, 10), 255);
    expect(_alpha(pixels, 10, 17), 0);
  });
}
