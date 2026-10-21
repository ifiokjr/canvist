import Foundation

/// How many times its budget a timing may take here: `CANVIST_PERF_SLACK`,
/// which the Rust, TypeScript, and Dart budgets read too. Shared CI runners
/// set it (`test.sh` forwards it to the simulator); elsewhere it is 1.
let perfSlack = ProcessInfo.processInfo.environment["CANVIST_PERF_SLACK"].flatMap(Double.init) ?? 1
