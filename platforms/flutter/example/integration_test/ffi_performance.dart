import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';

/// Registers a test that measures Dart → Rust round trips on a device, with
/// the same operations as `platforms/dart/benchmark/overhead.dart`, so the
/// numbers compare with the desktop ones. Results are printed as
/// `PERF dart …` lines; the assertions only guard against a broken
/// measurement.
///
/// This is a library rather than a `_test.dart` entrypoint: every entrypoint
/// makes `flutter test integration_test` build, install and launch the app
/// again, and on CI simulators that relaunch can hang. `canvist_test.dart`
/// registers it. Debug builds run Dart in JIT mode, so for representative
/// numbers run in profile mode on an Android emulator or a device:
/// `flutter drive --profile --driver test_driver/integration_test.dart --target integration_test/canvist_test.dart`.
void ffiPerformanceTests(Engine Function() engine) {
  test('command, query, and keystroke round trips', () {
    const samples = 200;
    const batch = 100;
    final doc = engine().parse(Format.markdown, 'The quick brown fox')
      ..select(const Position([0], 4));
    final commands = <double>[];
    final exports = <double>[];
    final stopwatch = Stopwatch();

    for (var sample = 0; sample < samples; sample++) {
      stopwatch
        ..reset()
        ..start();
      for (var call = 0; call < batch; call++) {
        doc.insertText('a');
      }
      stopwatch.stop();
      commands.add(_nanoseconds(stopwatch) / batch);

      stopwatch
        ..reset()
        ..start();
      for (var call = 0; call < batch; call++) {
        doc.export(Format.text);
      }
      stopwatch.stop();
      exports.add(_nanoseconds(stopwatch) / batch);
    }

    expect(doc.export(Format.text).length, 19 + samples * batch);
    doc.dispose();
    _report('insert_text', commands);
    _report('export_text', exports);
  });
}

double _nanoseconds(Stopwatch stopwatch) =>
    stopwatch.elapsedTicks * 1e9 / stopwatch.frequency;

void _report(String name, List<double> values) {
  values.sort();
  final median = values[values.length ~/ 2];
  final p90 = values[values.length * 9 ~/ 10];
  expect(median, greaterThan(0));
  const mode = kReleaseMode
      ? 'release'
      : kProfileMode
      ? 'profile'
      : 'debug';
  // ignore: avoid_print
  print(
    'PERF dart($mode) $name median=${median.round()}ns p90=${p90.round()}ns',
  );
}
