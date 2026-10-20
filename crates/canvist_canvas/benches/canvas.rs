//! Canvas hot-path benchmarks: index builds, hit tests, marquee, and a drag
//! step, at 1k, 10k, and 100k nodes. Run with `cargo bench -p canvist_canvas`.

#[path = "../tests/support/mod.rs"]
mod support;

use canvist_canvas::Canvas;
use canvist_canvas::MarqueeMode;
use canvist_canvas::Phase;
use canvist_canvas::PointerEvent;
use canvist_canvas::SceneIndex;
use divan::Bencher;
use divan::black_box;
use kurbo::Rect;

/// Node counts: frames of 100 rectangles.
const SIZES: [usize; 3] = [1_000, 10_000, 100_000];
/// The marquee's world rectangle: the first few frames of each scene.
const MARQUEE: Rect = Rect::new(0.0, 0.0, 5_000.0, 5_000.0);

fn main() {
	divan::main();
}

fn frames(nodes: usize) -> usize {
	(nodes / 100).max(1)
}

#[divan::bench(args = SIZES)]
fn build_index(bencher: Bencher<'_, '_>, nodes: usize) {
	let scene = support::framed(frames(nodes), support::RECTS_PER_FRAME);
	bencher.bench_local(|| {
		black_box(SceneIndex::build(&scene, &mut support::texts()).expect("index"))
	});
}

#[divan::bench(args = SIZES)]
fn hit_test(bencher: Bencher<'_, '_>, nodes: usize) {
	let scene = support::framed(frames(nodes), support::RECTS_PER_FRAME);
	let index = SceneIndex::build(&scene, &mut support::texts()).expect("index");
	bencher.bench_local(|| {
		black_box(canvist_canvas::hit_test(
			&scene,
			&index,
			black_box(support::ON_FIRST_RECT),
			4.0,
		))
	});
}

#[divan::bench(args = SIZES)]
fn marquee(bencher: Bencher<'_, '_>, nodes: usize) {
	let scene = support::framed(frames(nodes), support::RECTS_PER_FRAME);
	let index = SceneIndex::build(&scene, &mut support::texts()).expect("index");
	let rect = MARQUEE;
	bencher.bench_local(|| {
		black_box(canvist_canvas::marquee(
			&scene,
			&index,
			rect,
			MarqueeMode::Intersect,
		))
	});
}

#[divan::bench(args = SIZES)]
fn drag_step(bencher: Bencher<'_, '_>, nodes: usize) {
	let scene = support::framed(frames(nodes), support::RECTS_PER_FRAME);
	let mut builder = Canvas::builder().text_schema(support::text_schema());
	canvist_canvas::DesignKit::register(&mut builder);
	let mut canvas = builder.build(scene, support::texts()).expect("canvas");
	support::feed(
		&mut canvas,
		PointerEvent::mouse(Phase::Down, support::ON_FIRST_RECT, 0.0),
	);
	support::feed(
		&mut canvas,
		PointerEvent::mouse(Phase::Move, support::DRAG_START, 0.01),
	);
	let mut step = 0.0;

	bencher.bench_local(|| {
		step += 1.0;
		let position = support::wander(step);
		support::feed(
			&mut canvas,
			PointerEvent::mouse(Phase::Move, position, step),
		);
	});
}
