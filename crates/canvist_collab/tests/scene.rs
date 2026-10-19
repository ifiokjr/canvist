//! Scene collaboration: peers converge, conflicts resolve by the documented
//! rules, text frames merge character by character, and undo only reverts
//! this peer's work.

use std::sync::Arc;

use canvist_collab::Encoding;
use canvist_collab::LentDocument;
use canvist_collab::SceneBinding;
use canvist_collab::Track;
use canvist_model::BlockSpec;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_scene::Color;
use canvist_scene::NodeId;
use canvist_scene::Paint;
use canvist_scene::Prop;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::SceneSchema;
use canvist_scene::Transaction;
use kurbo::Affine;
use kurbo::Vec2;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn schema() -> Arc<SceneSchema> {
	let text = Arc::new(
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.build()
			.expect("text schema"),
	);
	Arc::new(SceneSchema::standard(text).expect("scene schema"))
}

/// One collaborator: a scene and its binding.
struct Peer {
	scene: Scene,
	binding: SceneBinding,
}

impl Peer {
	/// The first peer, seeding a new shared scene with one page.
	fn seed(schema: &Arc<SceneSchema>, client: u32) -> Self {
		let scene = Scene::new(schema.clone(), client).expect("scene");
		let mut binding =
			SceneBinding::with_client_id(schema.clone(), u64::from(client)).expect("binding");
		binding.seed(&scene).expect("seed");
		Self { scene, binding }
	}

	/// A peer joining from another's full state.
	fn join(schema: &Arc<SceneSchema>, client: u32, state: &[u8]) -> Self {
		let mut binding =
			SceneBinding::with_client_id(schema.clone(), u64::from(client)).expect("binding");
		binding.apply_state(state, Encoding::V1).expect("state");
		let scene = binding.scene(client).expect("join");
		Self { scene, binding }
	}

	fn page(&self) -> NodeId {
		self.scene.pages()[0]
	}

	/// Run `edit` in a transaction and mirror its commit.
	fn edit<T>(&mut self, edit: impl FnOnce(&mut Transaction<'_>) -> T) -> T {
		let mut transaction = self.scene.transaction();
		let value = edit(&mut transaction);
		let commit = transaction.commit();
		self.binding
			.apply_local(&mut self.scene, &commit, Track::Step)
			.expect("mirror local commit");
		value
	}

	fn receive(&mut self, updates: &[Vec<u8>]) {
		for update in updates {
			self.binding
				.apply_update(&mut self.scene, update, Encoding::V1, None)
				.expect("apply update");
		}

		assert_eq!(self.scene.check(), Ok(()));
	}
}

/// Two peers on one shared scene.
fn pair() -> (Peer, Peer) {
	let schema = schema();
	let ada = Peer::seed(&schema, 1);
	let state = ada.binding.encode_state(Encoding::V1);
	let grace = Peer::join(&schema, 2, &state);
	(ada, grace)
}

/// Exchange every pending update both ways.
fn sync(a: &mut Peer, b: &mut Peer) {
	let from_a = a.binding.take_updates();
	let from_b = b.binding.take_updates();
	b.receive(&from_a);
	a.receive(&from_b);
	// Receiving can repair duplicates, which produces more updates.
	let again_a = a.binding.take_updates();
	let again_b = b.binding.take_updates();
	b.receive(&again_a);
	a.receive(&again_b);
}

fn rect(transaction: &mut Transaction<'_>, parent: NodeId, name: &str) -> NodeId {
	transaction
		.insert(
			parent,
			None,
			"rectangle",
			Props {
				name: name.into(),
				size: Vec2::new(10.0, 10.0),
				..Props::default()
			},
		)
		.expect("insert rectangle")
}

#[test]
fn a_joining_peer_sees_the_seeded_scene() {
	let (ada, grace) = pair();

	assert_eq!(grace.scene.to_json(), ada.scene.to_json());
}

#[test]
fn inserts_moves_props_and_removes_converge() {
	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let (frame, shape) = ada.edit(|t| {
		let frame = t
			.insert(page, None, "frame", Props::default())
			.expect("frame");
		let shape = rect(t, page, "shape");
		(frame, shape)
	});
	sync(&mut ada, &mut grace);
	assert_eq!(grace.scene.to_json(), ada.scene.to_json());

	grace.edit(|t| {
		t.move_to(shape, frame, None).expect("move");
		t.set(
			shape,
			Prop::Fills(Arc::new([Paint::solid(Color::rgb(1, 2, 3))])),
		)
		.expect("fill");
	});
	sync(&mut ada, &mut grace);
	assert_eq!(
		ada.scene.node(shape).map(canvist_scene::Node::parent),
		Some(frame)
	);
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());

	ada.edit(|t| t.remove(&[frame]).expect("remove"));
	sync(&mut ada, &mut grace);
	assert!(!grace.scene.contains(shape));
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());
}

#[test]
fn concurrent_edits_to_different_props_both_survive() {
	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let shape = ada.edit(|t| rect(t, page, "shape"));
	sync(&mut ada, &mut grace);

	ada.edit(|t| t.set(shape, Prop::Opacity(0.5)).expect("opacity"));
	grace.edit(|t| {
		t.set(shape, Prop::Transform(Affine::translate((40.0, 0.0))))
			.expect("move");
	});
	sync(&mut ada, &mut grace);

	let props = ada.scene.node(shape).expect("shape").props().clone();
	assert!((props.opacity - 0.5).abs() < f64::EPSILON);
	assert_eq!(props.transform, Affine::translate((40.0, 0.0)));
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());
}

#[test]
fn a_child_added_to_a_deleted_parent_hides_until_the_delete_is_undone() {
	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let frame = ada.edit(|t| {
		t.insert(page, None, "frame", Props::default())
			.expect("frame")
	});
	sync(&mut ada, &mut grace);

	ada.edit(|t| t.remove(&[frame]).expect("remove"));
	let orphan = grace.edit(|t| rect(t, frame, "orphan"));
	sync(&mut ada, &mut grace);

	assert!(!ada.scene.contains(orphan));
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());
	assert_eq!(ada.binding.hidden_nodes(&ada.scene), 1);

	ada.binding.undo(&mut ada.scene, None).expect("undo");
	sync(&mut ada, &mut grace);
	assert_eq!(
		ada.scene.node(orphan).map(canvist_scene::Node::parent),
		Some(frame)
	);
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());
}

#[test]
fn concurrent_moves_that_form_a_cycle_converge_without_one() {
	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let (a, b) = ada.edit(|t| {
		let a = t.insert(page, None, "frame", Props::default()).expect("a");
		let b = t.insert(page, None, "frame", Props::default()).expect("b");
		(a, b)
	});
	sync(&mut ada, &mut grace);

	ada.edit(|t| t.move_to(a, b, None).expect("a into b"));
	grace.edit(|t| t.move_to(b, a, None).expect("b into a"));
	sync(&mut ada, &mut grace);

	assert_eq!(ada.scene.to_json(), grace.scene.to_json());
	assert_eq!(ada.scene.len(), 3, "both frames stay visible");
	let a_parent = ada.scene.node(a).map(canvist_scene::Node::parent);
	let b_parent = ada.scene.node(b).map(canvist_scene::Node::parent);
	assert!(a_parent == Some(b) || b_parent == Some(a));
}

#[test]
fn edits_inside_a_concurrently_moved_node_survive() {
	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let (frame, other) = ada.edit(|t| {
		let frame = t
			.insert(page, None, "frame", Props::default())
			.expect("frame");
		let other = t
			.insert(page, None, "frame", Props::default())
			.expect("other");
		(frame, other)
	});
	sync(&mut ada, &mut grace);

	ada.edit(|t| t.move_to(frame, other, None).expect("move"));
	let child = grace.edit(|t| rect(t, frame, "child"));
	sync(&mut ada, &mut grace);

	assert_eq!(
		ada.scene.node(child).map(canvist_scene::Node::parent),
		Some(frame)
	);
	assert_eq!(
		ada.scene.node(frame).map(canvist_scene::Node::parent),
		Some(other)
	);
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());
}

#[test]
fn text_frames_merge_concurrent_typing() {
	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let text = ada.edit(|t| {
		let text = t
			.insert(page, None, "text", Props::default())
			.expect("text");
		t.text(text, |inner| {
			let block = inner.doc().first_text_block().expect("paragraph");
			inner.insert_text(Point::new(block, 0), "Hello", MarkSet::empty())
		})
		.expect("type");
		text
	});
	sync(&mut ada, &mut grace);
	assert_eq!(
		grace
			.scene
			.text(text)
			.map(canvist_model::Document::plain_text)
			.as_deref(),
		Some("Hello")
	);

	ada.edit(|t| {
		t.text(text, |inner| {
			let block = inner.doc().first_text_block().expect("paragraph");
			inner.insert_text(Point::new(block, 5), " world", MarkSet::empty())
		})
		.expect("type")
	});
	grace.edit(|t| {
		t.text(text, |inner| {
			let block = inner.doc().first_text_block().expect("paragraph");
			inner.insert_text(Point::new(block, 0), ">> ", MarkSet::empty())
		})
		.expect("type")
	});
	sync(&mut ada, &mut grace);

	assert_eq!(
		ada.scene
			.text(text)
			.map(canvist_model::Document::plain_text)
			.as_deref(),
		Some(">> Hello world")
	);
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());
}

#[test]
fn undo_reverts_a_frame_and_its_text_together_and_spares_others() {
	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let theirs = grace.edit(|t| rect(t, page, "theirs"));
	let text = ada.edit(|t| {
		let text = t
			.insert(page, None, "text", Props::default())
			.expect("text");
		t.text(text, |inner| {
			let block = inner.doc().first_text_block().expect("paragraph");
			inner.insert_text(Point::new(block, 0), "Mine", MarkSet::empty())
		})
		.expect("type");
		text
	});
	sync(&mut ada, &mut grace);

	assert!(ada.binding.can_undo());
	ada.binding.undo(&mut ada.scene, None).expect("undo");
	sync(&mut ada, &mut grace);
	assert!(!ada.scene.contains(text));
	assert!(ada.scene.contains(theirs));
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());

	ada.binding.redo(&mut ada.scene, None).expect("redo");
	sync(&mut ada, &mut grace);
	assert_eq!(
		grace
			.scene
			.text(text)
			.map(canvist_model::Document::plain_text)
			.as_deref(),
		Some("Mine")
	);
}

#[test]
fn a_lent_text_frame_receives_remote_typing() -> TestResult {
	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let text = ada.edit(|t| {
		t.insert(page, None, "text", Props::default())
			.expect("text")
	});
	sync(&mut ada, &mut grace);

	// Grace's editor holds the frame while Ada types into it.
	let mut held = grace.scene.lend_text(text)?;
	ada.edit(|t| {
		t.text(text, |inner| {
			let block = inner.doc().first_text_block().expect("paragraph");
			inner.insert_text(Point::new(block, 0), "Hi", MarkSet::empty())
		})
		.expect("type")
	});

	for update in ada.binding.take_updates() {
		let mut lent = LentDocument {
			node: text,
			doc: &mut held,
		};
		grace
			.binding
			.apply_update(&mut grace.scene, &update, Encoding::V1, Some(&mut lent))?;
	}

	assert_eq!(held.plain_text(), "Hi");

	// Grace's editor types too, through the lent-text path.
	let block = held.first_text_block().expect("paragraph");
	let mut transaction = held.transaction();
	transaction.insert_text(Point::new(block, 2), "!", MarkSet::empty())?;
	let commit = transaction.commit();
	grace
		.binding
		.apply_local_text(text, &mut held, &commit, Track::Step)?;
	grace.scene.return_text(text, held)?;
	sync(&mut ada, &mut grace);

	assert_eq!(
		ada.scene
			.text(text)
			.map(canvist_model::Document::plain_text)
			.as_deref(),
		Some("Hi!")
	);
	assert_eq!(ada.scene.to_json(), grace.scene.to_json());
	Ok(())
}

#[test]
fn unknown_types_and_rejected_props_are_ignored_and_kept() -> TestResult {
	use yrs::Map;
	use yrs::Transact;

	let (mut ada, mut grace) = pair();
	let page = ada.page();
	let shape = ada.edit(|t| rect(t, page, "shape"));
	sync(&mut ada, &mut grace);

	// A peer with a richer schema writes a node type and a value this one
	// does not accept.
	{
		let doc = ada.binding.yrs();
		let nodes = doc.get_or_insert_map("nodes");
		let mut txn = doc.transact_mut();
		let Some(yrs::Out::YMap(node)) = nodes.get(&txn, &shape.to_string()) else {
			panic!("shape map");
		};
		let Some(yrs::Out::YMap(props)) = node.get(&txn, "props") else {
			panic!("props map");
		};
		props.insert(&mut txn, "opacity", 7.5);
		props.insert(&mut txn, "name", "renamed");
		let blob: yrs::MapRef = nodes.insert(&mut txn, "9.0", yrs::MapPrelim::default());
		blob.insert(&mut txn, "type", "hologram");
	}

	for update in ada.binding.take_updates() {
		grace
			.binding
			.apply_update(&mut grace.scene, &update, Encoding::V1, None)?;
	}

	let props = grace.scene.node(shape).expect("shape").props();
	assert!(
		(props.opacity - 1.0).abs() < f64::EPSILON,
		"out-of-range opacity is ignored"
	);
	assert_eq!(&*props.name, "renamed");
	assert_eq!(grace.binding.hidden_nodes(&grace.scene), 1);
	Ok(())
}

/// Two peers that seed the same scene under the same client id give every
/// prop the same item id, so when one edits a prop the other applies the
/// edit to that prop. (Their bytes can still differ: yrs encodes an
/// object value's keys in hash order.)
#[test]
fn seeding_the_same_scene_is_deterministic() -> TestResult {
	let schema = schema();
	let mut scene = Scene::new(schema.clone(), 1)?;
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let star = transaction.insert(
		page,
		None,
		"polygon",
		Props {
			name: "Star".into(),
			transform: Affine::rotate(0.2),
			size: Vec2::new(170.0, 170.0),
			opacity: 0.8,
			radius: [4.0; 4],
			fills: Arc::new([Paint::solid(Color::rgb(255, 200, 61))]),
			..Props::default()
		},
	)?;
	let _ = transaction.commit();

	let seed = || -> Result<Vec<u8>, canvist_collab::CollabError> {
		let mut binding = SceneBinding::with_client_id(schema.clone(), 1)?;
		binding.seed(&scene)?;
		Ok(binding.encode_state(Encoding::V1))
	};
	let mut ada = Peer::join(&schema, 2, &seed()?);
	let mut grace = Peer::join(&schema, 3, &seed()?);
	let edits = [
		Prop::Name("Moon".into()),
		Prop::Transform(Affine::translate((40.0, 0.0))),
		Prop::Size(Vec2::new(90.0, 90.0)),
		Prop::Opacity(0.5),
		Prop::Radius([9.0; 4]),
		Prop::Fills(Arc::new([Paint::solid(Color::rgb(59, 44, 245))])),
	];

	// Were any two seeded keys swapped, editing one of them would land on
	// the other for Grace.
	for prop in edits {
		ada.edit(|transaction| transaction.set(star, prop))?;
		let updates = ada.binding.take_updates();
		grace.receive(&updates);
		assert_eq!(grace.scene.to_json(), ada.scene.to_json());
	}

	Ok(())
}
