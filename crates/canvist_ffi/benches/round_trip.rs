//! Rust-only baseline for the FFI round-trip numbers measured on each
//! platform. It runs the same workload as the Swift, Kotlin and Dart
//! performance tests, without crossing a language boundary, so the platform
//! numbers minus this one is the binding overhead.
//!
//! Workload: per sample, a fresh handle receives `BATCH` single-character
//! appends (then `BATCH` calls to `text()`); the per-call time is the sample's
//! elapsed time divided by `BATCH`. Reports the median and p90 over `SAMPLES`.
//!
//! ```sh
//! cargo bench -p canvist_ffi --bench round_trip
//! ```

use std::hint::black_box;
use std::time::Instant;

use canvist_ffi::EditorHandle;

const WARMUP: usize = 1_000;
const SAMPLES: usize = 200;
const BATCH: u32 = 100;

fn main() {
	let warmup = EditorHandle::new();
	for offset in 0..WARMUP as u32 {
		warmup
			.insert_text(offset, "a")
			.expect("offset is the text length");
	}

	let mut inserts = Vec::with_capacity(SAMPLES);
	let mut reads = Vec::with_capacity(SAMPLES);
	for _ in 0..SAMPLES {
		let editor = EditorHandle::new();
		let start = Instant::now();
		for offset in 0..BATCH {
			black_box(&editor)
				.insert_text(offset, black_box("a"))
				.expect("offset is the text length");
		}
		inserts.push(start.elapsed().as_nanos() as f64 / f64::from(BATCH));

		let start = Instant::now();
		for _ in 0..BATCH {
			black_box(black_box(&editor).text());
		}
		reads.push(start.elapsed().as_nanos() as f64 / f64::from(BATCH));
	}

	report("insert_text", &mut inserts);
	report("text", &mut reads);
}

fn report(name: &str, samples: &mut [f64]) {
	samples.sort_by(f64::total_cmp);
	let median = samples[samples.len() / 2];
	let p90 = samples[samples.len() * 9 / 10];
	println!("PERF rust {name} median={median:.0}ns p90={p90:.0}ns");
}
