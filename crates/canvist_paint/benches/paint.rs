//! Painting benchmarks: the scene walk alone, GPU encoding, CPU
//! rasterisation, and SVG export, at 1k and 10k nodes with every node on
//! screen. Run with `cargo bench -p canvist_paint --features cpu,gpu`.

#[path = "../tests/support/mod.rs"]
mod support;

use canvist_canvas::Canvas;
use canvist_paint::Assets;
use canvist_paint::Recorder;
use divan::Bencher;
use divan::black_box;

/// Node counts.
const SIZES: [usize; 2] = [1_000, 10_000];
/// The side of each square, in world units.
const SIDE: f64 = 20.0;
/// A 1080p viewport.
const WIDTH: u32 = 1_920;
/// A 1080p viewport.
const HEIGHT: u32 = 1_080;
/// Zoomed out far enough that every node is on screen.
const ZOOM: f64 = 0.4;

fn main() {
	divan::main();
}

fn canvas(nodes: usize) -> Canvas {
	let mut canvas = support::canvas(support::grid(nodes, SIDE));
	canvas.state_mut().camera_mut().zoom = ZOOM;
	canvas
}

/// Paint `canvas`'s first page into `painter` at 1080p.
fn paint<P: canvist_paint::Painter>(canvas: &Canvas, assets: &Assets, painter: &mut P) {
	let sources = support::sources(canvas, assets);
	let view = support::view(canvas, f64::from(WIDTH), f64::from(HEIGHT));
	let page = canvas.state().scene().pages()[0];
	canvist_paint::paint_page(painter, &sources, page, &view);
}

/// The scene walk with a painter that only records: culling, level of
/// detail, and conversion, without any backend.
#[divan::bench(args = SIZES)]
fn walk(bencher: Bencher<'_, '_>, nodes: usize) {
	let canvas = canvas(nodes);
	let assets = Assets::new();
	bencher.bench_local(|| {
		let mut recorder = Recorder::new();
		paint(&canvas, &assets, &mut recorder);
		black_box(recorder.ops.len())
	});
}

/// Walking and encoding a `vello::Scene`, as every GPU frame does.
#[cfg(feature = "gpu")]
#[divan::bench(args = SIZES)]
fn encode_for_the_gpu(bencher: Bencher<'_, '_>, nodes: usize) {
	let canvas = canvas(nodes);
	let assets = Assets::new();
	let mut painter = canvist_paint::VelloPainter::new();
	bencher.bench_local(|| {
		painter.reset();
		paint(&canvas, &assets, &mut painter);
	});
}

/// The CPU fallback rasterising a 1080p frame.
#[cfg(feature = "cpu")]
#[divan::bench(args = SIZES)]
fn rasterise_on_the_cpu(bencher: Bencher<'_, '_>, nodes: usize) {
	let canvas = canvas(nodes);
	let assets = Assets::new();
	let mut painter = canvist_paint::CpuPainter::new(WIDTH, HEIGHT).expect("painter");
	bencher.bench_local(|| {
		painter.reset(WIDTH, HEIGHT).expect("reset");
		paint(&canvas, &assets, &mut painter);
		black_box(painter.render())
	});
}

/// Exporting the page as an SVG document.
#[divan::bench(args = SIZES)]
fn export_svg(bencher: Bencher<'_, '_>, nodes: usize) {
	let canvas = canvas(nodes);
	let assets = Assets::new();
	bencher.bench_local(|| {
		let mut painter =
			canvist_paint::SvgPainter::new(kurbo::Size::new(f64::from(WIDTH), f64::from(HEIGHT)));
		paint(&canvas, &assets, &mut painter);
		black_box(painter.finish())
	});
}
