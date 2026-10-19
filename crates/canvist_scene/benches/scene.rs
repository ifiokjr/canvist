//! Scene hot-path benchmarks.
//!
//! The headline numbers are `set_prop` and `move_in_frame`: the cost of one
//! drag or one property edit, which must stay flat as scenes grow. Run with
//! `cargo bench -p canvist_scene`.

#[path = "../tests/support/mod.rs"]
mod support;

use canvist_scene::NodeId;
use canvist_scene::Prop;
use canvist_scene::Scene;
use divan::Bencher;
use divan::black_box;
use kurbo::Affine;

/// Total node counts: frames of 100 rectangles each.
const SIZES: [usize; 3] = [100, 10_000, 100_000];

fn main() {
	divan::main();
}

fn fixture(nodes: usize) -> (Scene, Vec<NodeId>) {
	support::framed((nodes / 100).max(1), 100)
}

/// One transform change: a drag step on one node.
#[divan::bench(args = SIZES)]
fn set_prop(bencher: Bencher<'_, '_>, nodes: usize) {
	let (mut scene, frames) = fixture(nodes);
	let node = scene.children(frames[frames.len() / 2])[50];
	let mut x = 0.0;

	bencher.bench_local(|| {
		x += 1.0;
		let mut transaction = scene.transaction();
		transaction
			.set(node, Prop::Transform(Affine::translate((x, 4.0))))
			.expect("valid node");
		black_box(transaction.commit());
	});
}

/// Reorder one node within its frame (a layer-panel drag).
#[divan::bench(args = SIZES)]
fn move_in_frame(bencher: Bencher<'_, '_>, nodes: usize) {
	let (mut scene, frames) = fixture(nodes);
	let frame = frames[frames.len() / 2];
	let node = scene.children(frame)[10];
	let mut toggle = false;

	bencher.bench_local(|| {
		toggle = !toggle;
		let index = if toggle { 80 } else { 10 };
		let mut transaction = scene.transaction();
		transaction
			.move_to(node, frame, Some(index))
			.expect("valid move");
		black_box(transaction.commit());
	});
}

/// Move a node into another frame and back.
#[divan::bench(args = SIZES)]
fn reparent(bencher: Bencher<'_, '_>, nodes: usize) {
	let (mut scene, frames) = fixture(nodes);
	let (from, to) = (frames[0], frames[frames.len() - 1]);
	let node = scene.children(from)[0];

	bencher.bench_local(|| {
		let mut transaction = scene.transaction();
		transaction.move_to(node, to, None).expect("into");
		transaction.move_to(node, from, Some(0)).expect("back");
		black_box(transaction.commit());
	});
}

/// Insert one rectangle and remove it again.
#[divan::bench(args = SIZES)]
fn insert_and_remove(bencher: Bencher<'_, '_>, nodes: usize) {
	let (mut scene, frames) = fixture(nodes);
	let frame = frames[frames.len() / 2];

	bencher.bench_local(|| {
		let mut transaction = scene.transaction();
		let id = transaction
			.insert(frame, Some(50), "rectangle", support::rectangle_props(1))
			.expect("insert");
		transaction.remove(&[id]).expect("remove");
		black_box(transaction.commit());
	});
}

/// Build a scene of this many nodes in one transaction.
#[divan::bench(args = [1_000, 10_000])]
fn bulk_insert(bencher: Bencher<'_, '_>, nodes: usize) {
	bencher.bench_local(|| black_box(fixture(nodes)));
}

/// Serialize a scene to JSON.
#[divan::bench(args = [1_000, 10_000])]
fn to_json(bencher: Bencher<'_, '_>, nodes: usize) {
	let (scene, _) = fixture(nodes);
	bencher.bench_local(|| black_box(scene.to_json()));
}

/// Load a scene from JSON.
#[divan::bench(args = [1_000, 10_000])]
fn from_json(bencher: Bencher<'_, '_>, nodes: usize) {
	let (scene, _) = fixture(nodes);
	let json = scene.to_json();
	let schema = support::schema();

	bencher.bench_local(|| black_box(Scene::from_json(schema.clone(), 2, &json).expect("loads")));
}
