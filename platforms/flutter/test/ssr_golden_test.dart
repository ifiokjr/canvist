/// Golden tests against server rendering.
///
/// Server rendering lays documents out with parley and the bundled Atkinson
/// Hyperlegible font. Here the editor lays the same documents out with
/// Flutter's text engine and the same font, and both display lists are
/// painted by the editor's painter. Matching geometry and pixels mean a
/// page rendered on the server and the editor that replaces it show the
/// same lines in the same places.
///
/// On a mismatch both images are written to `build/ssr_golden/`.
library;

import 'dart:io';
import 'dart:ui' as ui;

import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

const _width = 480.0;
const _family = 'Atkinson Hyperlegible';
const _fonts = CanvistFonts.only(_family);

/// How far a text run may sit from where server rendering put it.
const _tolerance = 0.75;

/// The share of pixels that may differ, for antialiasing at fractional
/// positions.
const _pixelTolerance = 0.004;

const _fixtures = {
  'headings and paragraphs':
      '# The canvist editor\n\nA paragraph long enough to wrap onto a second line '
      'and a third, so line breaking is compared as well as glyph positions.\n\n'
      '## Second level\n\nShort one.',
  'marks':
      'Plain, **bold**, *italic*, ~~struck~~, `code`, and '
      '[a link](https://canvist.dev) in one wrapping paragraph of text.',
  'lists':
      '- First bullet\n- Second bullet that runs long enough to wrap onto '
      'its next line\n\n1. One\n2. Two\n\n- [x] Done\n- [ ] To do',
  'quotes and code':
      '> A quotation, set off from the text around it.\n\n```\nfn main() {\n'
      '    println!("hi");\n}\n```\n\nAfter the code.',
};

void main() {
  late Engine engine;

  setUpAll(() async {
    final bytes = File(
      '../../crates/canvist_headless/fonts/AtkinsonHyperlegible-Regular.ttf',
    ).readAsBytesSync();
    await (FontLoader(
      _family,
    )..addFont(Future.value(ByteData.sublistView(bytes)))).load();
    engine = (await Engine.starterKit()).engine;
  });

  for (final MapEntry(key: name, value: markdown) in _fixtures.entries) {
    testWidgets('$name match server rendering', (tester) async {
      final doc = engine.parse(Format.markdown, markdown);
      final view = doc.view(const ParagraphMeasurer(fonts: _fonts))
        ..setWidth(_width);
      addTearDown(() {
        view.dispose();
        doc.dispose();
      });

      final server = doc.serverDisplay(_width);
      final (:display, :height) = _layOutAll(view);
      final serverRuns = _runs(server);
      final runs = _runs(display);

      expect(
        runs.map((run) => run.text).toList(),
        serverRuns.map((run) => run.text).toList(),
        reason: 'the same text runs, so the same line breaks',
      );

      for (final (index, run) in runs.indexed) {
        final expected = serverRuns[index];
        expect(
          run.x,
          closeTo(expected.x, _tolerance),
          reason: '"${run.text}" x',
        );
        expect(
          run.baseline,
          closeTo(expected.baseline, _tolerance),
          reason: '"${run.text}" baseline',
        );
      }

      final serverFonts = [
        for (final font in server.fonts)
          FontSpec(
            family: font.family,
            size: font.size,
            weight: font.weight,
            italic: font.italic,
          ),
      ];
      final size = ui.Size(_width, height.ceilToDouble());
      final (expected, actual) = await tester.runAsync(
        () async => (
          await _render(server, serverFonts, size),
          await _render(display, view.fonts, size),
        ),
      ) as (Uint8List, Uint8List);
      final different = _differentPixels(expected, actual);

      if (different > _pixelTolerance) {
        await tester.runAsync(() async {
          await _save('$name server', server, serverFonts, size);
          await _save('$name editor', display, view.fonts, size);
        });
      }

      expect(
        different,
        lessThanOrEqualTo(_pixelTolerance),
        reason: 'share of pixels that differ',
      );
    });
  }
}

/// Lay out the whole document and return what to paint.
({ViewDisplay display, double height}) _layOutAll(EditorView view) {
  var height = 1.0;
  ViewDisplay? display;

  for (var round = 0; round < 4; round++) {
    final frame = view.frame(0, height);
    display = frame.display ?? display;

    if (frame.height == height) {
      break;
    }

    height = frame.height;
  }

  return (display: display!, height: height);
}

List<ViewItem> _runs(ViewDisplay display) => [
  for (final item in display.items)
    if (item.kind == ViewItemKind.text) item,
];

Future<Uint8List> _render(
  ViewDisplay display,
  List<FontSpec> fonts,
  ui.Size size,
) async {
  final image = await _image(display, fonts, size);
  final bytes = await image.toByteData();
  image.dispose();
  return bytes!.buffer.asUint8List();
}

Future<ui.Image> _image(
  ViewDisplay display,
  List<FontSpec> fonts,
  ui.Size size,
) {
  final recorder = ui.PictureRecorder();
  final canvas = ui.Canvas(recorder)
    ..drawRect(
      ui.Offset.zero & size,
      ui.Paint()..color = ui.Color(display.background),
    );
  DisplayPainter(fonts: _fonts).paint(canvas, display, fonts);
  return recorder.endRecording().toImage(size.width.ceil(), size.height.ceil());
}

/// The share of pixels where a channel differs by more than a quarter.
double _differentPixels(Uint8List a, Uint8List b) {
  var different = 0;

  for (var pixel = 0; pixel < a.length; pixel += 4) {
    for (var channel = 0; channel < 3; channel++) {
      if ((a[pixel + channel] - b[pixel + channel]).abs() > 64) {
        different++;
        break;
      }
    }
  }

  return different / (a.length / 4);
}

Future<void> _save(
  String name,
  ViewDisplay display,
  List<FontSpec> fonts,
  ui.Size size,
) async {
  final image = await _image(display, fonts, size);
  final png = await image.toByteData(format: ui.ImageByteFormat.png);
  image.dispose();
  final file = File('build/ssr_golden/${name.replaceAll(' ', '_')}.png');
  await file.parent.create(recursive: true);
  await file.writeAsBytes(png!.buffer.asUint8List());
}
