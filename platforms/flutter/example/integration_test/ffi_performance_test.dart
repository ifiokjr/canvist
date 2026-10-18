import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

/// Measures one Dart → Rust round trip with the same workload as
/// `crates/canvist_ffi/benches/round_trip.rs`, so the difference is the cost
/// of the flutter_rust_bridge binding. Results are printed as `PERF dart …`
/// lines; the assertions only guard against a broken measurement.
///
/// Debug builds run Dart in JIT mode, so for representative numbers run this
/// file in profile mode on an Android emulator or a device:
/// `flutter drive --profile --driver test_driver/integration_test.dart --target integration_test/ffi_performance_test.dart`.
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustLib.init);

  test('insert_text and text round trips', () {
    const warmup = 1000;
    const samples = 200;
    const batch = 100;

    final warm = EditorHandle();
    for (var offset = 0; offset < warmup; offset++) {
      warm.insertText(offset: offset, text: 'a');
    }
    warm.dispose();

    final inserts = <double>[];
    final reads = <double>[];
    final stopwatch = Stopwatch();
    for (var sample = 0; sample < samples; sample++) {
      final editor = EditorHandle();
      stopwatch
        ..reset()
        ..start();
      for (var offset = 0; offset < batch; offset++) {
        editor.insertText(offset: offset, text: 'a');
      }
      stopwatch.stop();
      inserts.add(_nanoseconds(stopwatch) / batch);

      stopwatch
        ..reset()
        ..start();
      for (var call = 0; call < batch; call++) {
        editor.text();
      }
      stopwatch.stop();
      reads.add(_nanoseconds(stopwatch) / batch);
      expect(editor.text().length, batch);
      editor.dispose();
    }

    _report('insert_text', inserts);
    _report('text', reads);
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
