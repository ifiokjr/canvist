import Canvist
import Dispatch
import XCTest

/// Measures one Swift → Rust round trip with the same workload as
/// `crates/canvist_ffi/benches/round_trip.rs`, so the difference is the cost
/// of the UniFFI binding. Results are printed as `PERF swift …` lines; the
/// assertions only guard against a broken measurement, not a slow machine.
final class FFIPerformanceTests: XCTestCase {
    private let warmup = 1_000
    private let samples = 200
    private let batch: UInt32 = 100

    func testInsertTextAndTextRoundTrips() throws {
        let warm = EditorHandle()
        for offset in 0..<UInt32(warmup) {
            try warm.insertText(offset: offset, text: "a")
        }

        var inserts: [Double] = []
        var reads: [Double] = []
        for _ in 0..<samples {
            let editor = EditorHandle()
            var start = DispatchTime.now().uptimeNanoseconds
            for offset in 0..<batch {
                try editor.insertText(offset: offset, text: "a")
            }
            inserts.append(Double(DispatchTime.now().uptimeNanoseconds - start) / Double(batch))

            start = DispatchTime.now().uptimeNanoseconds
            for _ in 0..<batch {
                _ = editor.text()
            }
            reads.append(Double(DispatchTime.now().uptimeNanoseconds - start) / Double(batch))
            XCTAssertEqual(editor.text().count, Int(batch))
        }

        report("insert_text", inserts)
        report("text", reads)
    }

    private func report(_ name: String, _ values: [Double]) {
        let sorted = values.sorted()
        let median = sorted[sorted.count / 2]
        let p90 = sorted[sorted.count * 9 / 10]
        XCTAssertGreaterThan(median, 0)
        #if DEBUG
            let configuration = "debug"
        #else
            let configuration = "release"
        #endif
        print("PERF swift(\(configuration)) \(name) median=\(Int(median))ns p90=\(Int(p90))ns")
    }
}
