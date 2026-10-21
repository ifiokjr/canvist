/// What a call through flutter_rust_bridge costs, measured on the same
/// operations as `crates/canvist_headless/tests/perf_budgets.rs`, so the
/// difference is the binding's overhead, plus an editor view's keystroke to
/// frame.
///
/// ```sh
/// dart run benchmark/overhead.dart           # print the numbers
/// dart run benchmark/overhead.dart --check   # fail when one is over budget
/// ```
///
/// `CANVIST_PERF_SLACK` multiplies every budget, as for the Rust budgets.
library;

import 'dart:io';
import 'dart:typed_data';

import 'package:canvist/canvist.dart';

const _paragraph =
    'The quick brown fox jumps over the lazy dog, again and again, until the line wraps.';
const _paragraphs = 10000;
const _samples = 15;
const _caret = Position([_paragraphs ~/ 2], 10);

final _slack =
    double.tryParse(Platform.environment['CANVIST_PERF_SLACK'] ?? '') ?? 1;
final _failures = <String>[];

/// The median time of one call, in microseconds, over [_samples] batches.
double _perCall(int batch, void Function() run) {
  run();
  final samples = List.generate(_samples, (_) {
    final watch = Stopwatch()..start();

    for (var index = 0; index < batch; index += 1) {
      run();
    }

    return watch.elapsedMicroseconds / batch;
  })..sort();

  return samples[_samples ~/ 2];
}

void _report(String name, double measured, double budget, String unit) {
  final over = measured > budget * _slack;
  final digits = unit == 'µs' ? 2 : 1;
  stdout.writeln(
    '$name: ${measured.toStringAsFixed(digits)} $unit (budget $budget $unit)${over ? '  OVER' : ''}',
  );

  if (over) {
    _failures.add(name);
  }
}

Future<void> main(List<String> arguments) async {
  final (:engine, :kit) = await Engine.starterKit();
  final markdown = List.generate(
    _paragraphs,
    (index) => '$_paragraph $index',
  ).join('\n\n');
  final doc = engine.parse(Format.markdown, markdown)..select(_caret);
  final toggle = kit.bold.toggleBold();
  final isBold = kit.bold.isBold();

  _report(
    'run a typed command @10k paragraphs',
    _perCall(2000, () => doc.run(toggle)),
    10,
    'µs',
  );
  _report(
    'answer a query @10k paragraphs',
    _perCall(2000, () => doc.query(isBold)),
    10,
    'µs',
  );

  var typing = false;
  _report(
    'type or delete a character @10k paragraphs',
    _perCall(2000, () {
      typing = !typing;
      typing ? doc.insertText('x') : doc.press('Backspace');
    }),
    10,
    'µs',
  );

  final server = engine.parse(Format.markdown, markdown)..share(1);
  final peer = engine.fromYjs(server.encodeState(), clientId: 2);
  final updates = <Uint8List>[];

  for (var index = 0; index < (_samples + 1) * 200; index += 1) {
    peer
      ..select(_caret)
      ..insertText('y')
      ..press('Backspace');
    updates.addAll(peer.takeUpdates());
  }

  final incoming = updates.iterator;
  _report(
    'apply one Yjs keystroke update @10k paragraphs',
    _perCall(200, () {
      incoming.moveNext();
      server.applyUpdate(incoming.current);
    }),
    30,
    'µs',
  );

  _report(
    'load 10k paragraphs from Markdown',
    _perCall(1, () => engine.parse(Format.markdown, markdown)) / 1000,
    100,
    'ms',
  );
  _report(
    'export 10k paragraphs as JSON',
    _perCall(1, () => doc.export(Format.json)) / 1000,
    100,
    'ms',
  );

  // An editor's keystroke: the edit, then the frame that paints it, laid
  // out to the viewport. Measurement is cached after the first frame, so
  // this is the layout, display list, and bridge cost a host pays per key.
  final viewed = engine.parse(Format.markdown, markdown)
    ..select(const Position([3], 10));
  final view = viewed.view(const FixedMeasurer())
    ..setWidth(720)
    ..setFocused(true);
  view.frame(0, 900);
  var viewTyping = false;
  _report(
    'keystroke to frame in a view @10k paragraphs',
    _perCall(500, () {
      viewTyping = !viewTyping;
      viewTyping ? view.insertText('x') : view.keyDown('Backspace');

      if (view.frame(0, 900).display == null) {
        throw StateError('the keystroke changed nothing on screen');
      }
    }),
    250,
    'µs',
  );
  _report(
    'frame with nothing changed @10k paragraphs',
    _perCall(2000, () => view.frame(0, 900)),
    100,
    'µs',
  );
  view.dispose();

  if (arguments.contains('--check') && _failures.isNotEmpty) {
    stderr.writeln('over budget: ${_failures.join(', ')}');
    exitCode = 1;
  }
}
