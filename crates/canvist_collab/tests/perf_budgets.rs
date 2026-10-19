//! Performance budgets for the collaboration hot paths.
//!
//! These tests fail when an operation exceeds the budget recorded in
//! `docs/src/guides/performance.md`. They only mean something in release mode:
//!
//! ```sh
//! cargo test --release -p canvist_collab --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget. CI sets it for noisy shared
//! runners; locally it defaults to 1.
//!
//! The file compiles to nothing in debug builds, where timings are meaningless.

#![cfg(not(debug_assertions))]

mod support;

use std::time::Duration;
use std::time::Instant;

use canvist_collab::Binding;
use canvist_collab::Encoding;
use canvist_collab::Track;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Point;

/// Samples per measurement; the median is reported.
const SAMPLES: usize = 31;
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const LARGE_DOC: usize = 100_000;
/// Blocks at the root for the structural budgets. Enter costs O(siblings):
/// yrs computes an array event's delta by walking the whole array (Yjs does
/// the same) and walks to an insertion index. At 100,000 root blocks that
/// walk is bound by memory latency and varies about 14 times between
/// machines (about 9 ns a block on CI's x86 runners, 130 ns on Apple
/// silicon), so a budget there could not be both tight and portable. At
/// 10,000 blocks the arrays stay in cache everywhere.
const ROOT_SIBLINGS: usize = 10_000;
const JOINED_DOC: usize = 10_000;
/// About 3.5 times the 114 µs measured on Apple silicon.
const REMOTE_ENTER_MICROS: u64 = 400;
/// About 3.5 times the 45 µs measured on Apple silicon.
const ENCODE_ENTER_MICROS: u64 = 150;
/// Keystrokes per sample. All land in one paragraph, which grows by about
/// 3,000 characters over a run: long, but a paragraph people write.
const KEYSTROKES: u32 = 100;

fn slack() -> f64 {
	let configured = std::env::var("CANVIST_PERF_SLACK").ok();
	configured
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

/// The median time of one call to `run`, measured over [`SAMPLES`] batches of
/// `batch` calls after one warm-up batch.
fn median_per_call(batch: u32, mut run: impl FnMut()) -> Duration {
	for _ in 0..batch {
		run();
	}

	let mut samples: Vec<Duration> = (0..SAMPLES)
		.map(|_| {
			let start = Instant::now();

			for _ in 0..batch {
				run();
			}

			start.elapsed() / batch
		})
		.collect();
	samples.sort_unstable();
	samples[SAMPLES / 2]
}

fn assert_within(name: &str, measured: Duration, budget: Duration) {
	let allowed = budget.mul_f64(slack());
	eprintln!("{name}: {measured:?} (budget {budget:?}, allowed {allowed:?})");
	assert!(
		measured <= allowed,
		"{name} took {measured:?}, over its {allowed:?} budget"
	);
}

/// A document of `blocks` paragraphs, seeded into a binding.
fn seeded(blocks: usize) -> (Document, Binding) {
	let schema = support::schema();
	let paragraph = schema.default_text_block();
	let mut doc = Document::empty(schema.clone(), 1);
	let trees = (0..blocks)
		.map(|index| {
			let id = BlockId::new(1, u32::try_from(index).expect("fits"));
			let text = InlineText::plain(PARAGRAPH, MarkSet::empty());
			BlockTree::text(id, paragraph, text)
		})
		.collect();
	doc.transaction()
		.insert_blocks(None, 0, trees)
		.expect("paragraphs");
	let mut binding = Binding::with_client_id(schema, 1).expect("binding");
	binding.seed(&doc).expect("seed");
	binding.take_updates();
	(doc, binding)
}

/// A second peer that joined `binding`'s document.
fn follower(binding: &Binding) -> (Document, Binding) {
	let schema = support::schema();
	let mut follower = Binding::with_client_id(schema.clone(), 2).expect("binding");
	let mut doc = Document::empty(schema, 2);
	let state = binding.encode_state(Encoding::V1);
	follower
		.apply_update(&mut doc, &state, Encoding::V1)
		.expect("join");
	(doc, follower)
}

/// Type one character at the end of the middle block, as typing does, and
/// return its update.
fn keystroke(doc: &mut Document, binding: &mut Binding) -> Vec<u8> {
	let block = doc.children(None)[doc.children(None).len() / 2];
	let end = doc.inline(block).map_or(0, InlineText::len);
	let mut transaction = doc.transaction();
	transaction
		.insert_text(Point::new(block, end), "x", MarkSet::empty())
		.expect("insert");
	let commit = transaction.commit();
	binding
		.apply_local(doc, &commit, Track::Merge)
		.expect("local");
	binding.take_updates().concat()
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn encode_a_keystroke_in_a_100k_block_document() {
	let (mut doc, mut binding) = seeded(LARGE_DOC);

	let measured = median_per_call(KEYSTROKES, || {
		std::hint::black_box(keystroke(&mut doc, &mut binding));
	});

	assert_within(
		"encode keystroke @100k blocks",
		measured,
		Duration::from_micros(20),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn apply_a_remote_keystroke_in_a_100k_block_document() {
	let (mut doc, mut binding) = seeded(LARGE_DOC);
	let (mut mirror, mut receiver) = follower(&binding);
	let batch = KEYSTROKES as usize;
	let updates: Vec<Vec<u8>> = (0..batch * (SAMPLES + 1))
		.map(|_| keystroke(&mut doc, &mut binding))
		.collect();
	let mut updates = updates.into_iter();

	let measured = median_per_call(batch as u32, || {
		let update = updates.next().expect("one update per call");
		let commit = receiver
			.apply_update(&mut mirror, &update, Encoding::V1)
			.expect("remote");
		std::hint::black_box(commit);
	});

	assert_within(
		"apply remote keystroke @100k blocks",
		measured,
		Duration::from_micros(20),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn apply_a_remote_enter_among_10k_blocks() {
	let (mut doc, mut binding) = seeded(ROOT_SIBLINGS);
	let (mut mirror, mut receiver) = follower(&binding);
	let paragraph = doc.schema().default_text_block();
	let batch = 20;
	let updates: Vec<Vec<u8>> = (0..batch * (SAMPLES + 1))
		.map(|_| {
			let block = doc.children(None)[ROOT_SIBLINGS / 2];
			let mut transaction = doc.transaction();
			transaction
				.split(Point::new(block, 20), paragraph, Attrs::new())
				.expect("enter");
			let commit = transaction.commit();
			binding
				.apply_local(&mut doc, &commit, Track::Step)
				.expect("local");
			binding.take_updates().concat()
		})
		.collect();
	let mut updates = updates.into_iter();

	let measured = median_per_call(batch as u32, || {
		let update = updates.next().expect("one update per call");
		let commit = receiver
			.apply_update(&mut mirror, &update, Encoding::V1)
			.expect("remote");
		std::hint::black_box(commit);
	});

	assert_within(
		"apply remote enter @10k blocks",
		measured,
		Duration::from_micros(REMOTE_ENTER_MICROS),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn encode_an_enter_among_10k_blocks() {
	let (mut doc, mut binding) = seeded(ROOT_SIBLINGS);
	let paragraph = doc.schema().default_text_block();
	let block = doc.children(None)[ROOT_SIBLINGS / 2];

	let measured = median_per_call(20, || {
		let mut transaction = doc.transaction();
		transaction
			.split(Point::new(block, 20), paragraph, Attrs::new())
			.expect("enter");
		let commit = transaction.commit();
		binding
			.apply_local(&mut doc, &commit, Track::Step)
			.expect("local");
		std::hint::black_box(binding.take_updates());
	});

	assert_within(
		"encode enter @10k blocks",
		measured,
		Duration::from_micros(ENCODE_ENTER_MICROS),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn join_a_10k_paragraph_document() {
	let (_, binding) = seeded(JOINED_DOC);

	let measured = median_per_call(1, || {
		std::hint::black_box(follower(&binding));
	});

	// The whole "open a 10k-paragraph document → first paint" budget, since
	// joining from Yjs state replaces loading the model from JSON. About
	// half goes to yrs decoding and integrating the update; the binding's
	// mirror is the rest.
	assert_within("join 10k paragraphs", measured, Duration::from_millis(50));
}

/// Scene budgets: one property change crossing the wire at 10,000 nodes.
#[cfg(feature = "scene")]
mod scene {
	use std::sync::Arc;
	use std::time::Duration;

	use canvist_collab::Encoding;
	use canvist_collab::SceneBinding;
	use canvist_collab::Track;
	use canvist_model::BlockSpec;
	use canvist_model::Schema;
	use canvist_scene::NodeId;
	use canvist_scene::Prop;
	use canvist_scene::Props;
	use canvist_scene::Scene;
	use canvist_scene::SceneSchema;
	use kurbo::Affine;
	use kurbo::Vec2;

	use super::assert_within;
	use super::median_per_call;

	const FRAMES: usize = 100;
	const PER_FRAME: usize = 99;
	const BATCH: u32 = 200;

	fn schema() -> Arc<SceneSchema> {
		let text = Arc::new(
			Schema::builder()
				.block(BlockSpec::text("paragraph"))
				.build()
				.expect("text schema"),
		);
		Arc::new(SceneSchema::standard(text).expect("scene schema"))
	}

	/// Two synced peers sharing a 10k-node scene, and one rectangle in it.
	fn pair() -> ((Scene, SceneBinding), (Scene, SceneBinding), NodeId) {
		let schema = schema();
		let mut scene = Scene::new(schema.clone(), 1).expect("scene");
		let page = scene.pages()[0];
		let mut transaction = scene.transaction();
		let mut target = page;

		for frame in 0..FRAMES {
			let id = transaction
				.insert(page, None, "frame", Props::default())
				.expect("frame");

			for index in 0..PER_FRAME {
				let props = Props {
					size: Vec2::new(10.0, 10.0),
					..Props::default()
				};
				let rect = transaction
					.insert(id, None, "rectangle", props)
					.expect("rect");

				if frame == FRAMES / 2 && index == PER_FRAME / 2 {
					target = rect;
				}
			}
		}

		let _ = transaction.commit();
		let mut binding = SceneBinding::with_client_id(schema.clone(), 1).expect("binding");
		binding.seed(&scene).expect("seed");
		// The follower joins from the full state, which includes the seed.
		let _ = binding.take_updates();
		let mut follower = SceneBinding::with_client_id(schema, 2).expect("binding");
		follower
			.apply_state(&binding.encode_state(Encoding::V1), Encoding::V1)
			.expect("state");
		let follower_scene = follower.scene(2).expect("join");
		((scene, binding), (follower_scene, follower), target)
	}

	/// Move `node` and return the one update that carries it.
	fn nudge(scene: &mut Scene, binding: &mut SceneBinding, node: NodeId, x: f64) -> Vec<u8> {
		let mut transaction = scene.transaction();
		transaction
			.set(node, Prop::Transform(Affine::translate((x, 0.0))))
			.expect("move");
		let commit = transaction.commit();
		binding
			.apply_local(scene, &commit, Track::Merge)
			.expect("encode");
		let mut updates = binding.take_updates();
		assert_eq!(updates.len(), 1, "one commit makes one update");
		updates.pop().unwrap_or_default()
	}

	#[test]
	#[ignore = "release-mode performance budget; run with `test:perf`"]
	fn encode_a_property_change_in_a_10k_node_scene() {
		let ((mut scene, mut binding), _, node) = pair();
		let mut x = 0.0;

		let measured = median_per_call(BATCH, || {
			x += 1.0;
			std::hint::black_box(nudge(&mut scene, &mut binding, node, x));
		});

		assert_within(
			"encode a scene prop change @10k nodes",
			measured,
			Duration::from_micros(50),
		);
	}

	#[test]
	#[ignore = "release-mode performance budget; run with `test:perf`"]
	fn apply_a_remote_property_change_in_a_10k_node_scene() {
		let ((mut scene, mut binding), (mut follower_scene, mut follower), node) = pair();
		let total = BATCH as usize * (super::SAMPLES + 1);
		let mut updates: Vec<Vec<u8>> = (0..total)
			.map(|step| nudge(&mut scene, &mut binding, node, step as f64))
			.collect();
		updates.reverse();

		let measured = median_per_call(BATCH, || {
			let update = updates.pop().expect("enough updates");
			let commit = follower
				.apply_update(&mut follower_scene, &update, Encoding::V1, None)
				.expect("apply");
			std::hint::black_box(commit);
		});

		assert!(!follower.is_waiting(), "every update applied");
		assert_eq!(
			follower_scene
				.node(node)
				.map(|found| found.props().transform),
			scene.node(node).map(|found| found.props().transform),
		);
		assert_within(
			"apply a remote scene prop change @10k nodes",
			measured,
			Duration::from_micros(50),
		);
	}
}
