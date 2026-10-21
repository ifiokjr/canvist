/// The editor on a device draws what server rendering draws, pixel for
/// pixel.
///
/// Server rendering lays documents out with parley and Atkinson Hyperlegible.
/// Here the real [CanvistEditor] lays the same documents out with the
/// device's text engine and the same font, bundled with the app, and draws
/// them through its render object at the device's pixel ratio. Its pixels
/// are compared with the server's display list rasterised by the same GPU
/// backend, so any difference comes from where text and boxes are, not from
/// how the device antialiases.
library;

import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart' show expect, lessThanOrEqualTo;
import 'package:patrol/patrol.dart';

/// Narrower than any phone, so the editor never overflows the screen.
const _width = 300.0;
const _fonts = CanvistFonts.only('Atkinson Hyperlegible');

/// The share of pixels that may differ, for antialiasing at fractional
/// positions.
const _pixelTolerance = 0.004;

/// How far a colour channel may move, out of 255, before the pixel counts
/// as different.
const _channel = 64;

const _fixtures = {
  'headings and paragraphs':
      '# The canvist editor\n\nA paragraph long enough to wrap onto a second '
      'line and a third, so line breaking is compared too.\n\n'
      '## Second level\n\nShort one.',
  'marks':
      'Plain, **bold**, *italic*, ~~struck~~, `code`, and '
      '[a link](https://canvist.dev) in one wrapping paragraph.',
  'lists':
      '- First bullet\n- Second bullet that runs long enough to wrap\n\n'
      '1. One\n2. Two\n\n- [x] Done\n- [ ] To do',
  'quotes and code':
      '> A quotation, set off from the text around it.\n\n```\nfn main() {\n'
      '    println!("hi");\n}\n```\n\nAfter the code.',
};

void main() {
  for (final MapEntry(key: name, value: markdown) in _fixtures.entries) {
    patrolTest(
      '$name draw as server rendering does',
      ($) => _compare($, name, markdown),
    );
  }
}

Future<void> _compare(
  PatrolIntegrationTester $,
  String name,
  String markdown,
) async {
  final (:engine, kit: _) = await Engine.starterKit();
  final doc = engine.parse(Format.markdown, markdown);
  final controller = CanvistController(engine: engine, doc: doc, fonts: _fonts);
  final boundary = GlobalKey();

  await $.pumpWidgetAndSettle(
    MaterialApp(
      debugShowCheckedModeBanner: false,
      home: Scaffold(
        body: UnconstrainedBox(
          alignment: Alignment.topLeft,
          child: SizedBox(
            width: _width,
            child: RepaintBoundary(
              key: boundary,
              child: CanvistEditor(
                controller: controller,
                padding: EdgeInsets.zero,
                readOnly: true,
              ),
            ),
          ),
        ),
      ),
    ),
  );

  final render =
      boundary.currentContext!.findRenderObject()! as RenderRepaintBoundary;
  final ratio = $.tester.view.devicePixelRatio;
  final server = doc.serverDisplay(_width);
  final widget = await render.toImage(pixelRatio: ratio);
  final expected = await _rasterise(server, render.size, ratio);
  final width = widget.width;
  // The widget is as tall as its document, a fractional height, and paints
  // only its own bounds: compare the pixel rows it covers fully.
  final rows = (render.size.height * ratio).floor();
  final actual = await _rgba(widget);
  final reference = await _rgba(expected);
  final different = _share(reference, actual, width, rows);

  controller.dispose();
  doc.dispose();
  expect(
    different,
    lessThanOrEqualTo(_pixelTolerance),
    reason:
        '$name: share of pixels that differ from server rendering at '
        '${ratio}x',
  );
}

/// The server's display list painted by the editor's painter, as the
/// widget would be at [ratio] device pixels per logical pixel.
Future<ui.Image> _rasterise(ViewDisplay display, Size size, double ratio) {
  final recorder = ui.PictureRecorder();
  final canvas = Canvas(recorder)
    ..scale(ratio)
    ..drawRect(Offset.zero & size, Paint()..color = Color(display.background));
  final fonts = [
    for (final font in display.fonts)
      FontSpec(
        family: font.family,
        size: font.size,
        weight: font.weight,
        italic: font.italic,
      ),
  ];
  DisplayPainter(fonts: _fonts).paint(canvas, display, fonts);

  return recorder.endRecording().toImage(
    (size.width * ratio).ceil(),
    (size.height * ratio).ceil(),
  );
}

Future<Uint8List> _rgba(ui.Image image) async {
  final bytes = await image.toByteData();
  image.dispose();
  return bytes!.buffer.asUint8List();
}

/// The share of the first [rows] rows' pixels where a channel moved more
/// than [_channel].
double _share(Uint8List a, Uint8List b, int width, int rows) {
  var different = 0;
  final end = width * rows * 4;

  for (var pixel = 0; pixel < end; pixel += 4) {
    for (var channel = 0; channel < 3; channel++) {
      if ((a[pixel + channel] - b[pixel + channel]).abs() > _channel) {
        different++;
        break;
      }
    }
  }

  return different / (width * rows);
}
