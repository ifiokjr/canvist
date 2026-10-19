//! Peers editing one scene concurrently converge.
//!
//! Each case starts three peers from one seeded scene. They insert, remove,
//! move, restyle, type into text frames, and undo, while updates arrive in
//! random order and some arrive twice. After everything is delivered every
//! peer's scene must be valid, equal every other peer's, and equal a scene
//! rebuilt from scratch out of the shared state.

use std::sync::Arc;

use canvist_collab::Encoding;
use canvist_collab::SceneBinding;
use canvist_collab::Track;
use canvist_model::BlockSpec;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_scene::NodeId;
use canvist_scene::Prop;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::SceneSchema;
use canvist_scene::Transaction;
use kurbo::Affine;
use proptest::prelude::*;

const PEERS: usize = 3;
const KINDS: [&str; 4] = ["frame", "group", "rectangle", "text"];

fn schema() -> Arc<SceneSchema> {
	let text = Arc::new(
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.build()
			.expect("text schema"),
	);
	Arc::new(SceneSchema::standard(text).expect("scene schema"))
}

#[derive(Clone, Copy, Debug)]
enum Op {
	Insert { parent: u8, kind: u8 },
	Remove { node: u8 },
	Move { node: u8, parent: u8, front: bool },
	Restyle { node: u8, value: u8 },
	Type { node: u8, at: u8 },
	Undo,
}

#[derive(Clone, Copy, Debug)]
enum Event {
	Edit(usize, Op),
	Deliver(u32, bool),
}

fn op() -> impl Strategy<Value = Op> {
	prop_oneof![
		4 => (any::<u8>(), any::<u8>()).prop_map(|(parent, kind)| Op::Insert { parent, kind }),
		1 => any::<u8>().prop_map(|node| Op::Remove { node }),
		3 => (any::<u8>(), any::<u8>(), any::<bool>()).prop_map(|(node, parent, front)| Op::Move { node, parent, front }),
		3 => (any::<u8>(), any::<u8>()).prop_map(|(node, value)| Op::Restyle { node, value }),
		3 => (any::<u8>(), any::<u8>()).prop_map(|(node, at)| Op::Type { node, at }),
		1 => Just(Op::Undo),
	]
}

fn event() -> impl Strategy<Value = Event> {
	prop_oneof![
		3 => (0..PEERS, op()).prop_map(|(peer, op)| Event::Edit(peer, op)),
		2 => (any::<u32>(), prop::bool::weighted(0.2)).prop_map(|(choice, twice)| Event::Deliver(choice, twice)),
	]
}

struct Peer {
	scene: Scene,
	binding: SceneBinding,
}

impl Peer {
	fn pick(&self, seed: u8) -> NodeId {
		let all = self.scene.subtree(NodeId::ROOT);
		all[usize::from(seed) % all.len()]
	}

	/// Apply one op; ops that the scene rejects are skipped.
	fn apply(&mut self, op: Op) {
		if let Op::Undo = op {
			self.binding.undo(&mut self.scene, None).expect("undo");
			return;
		}

		if self.scene.is_empty() {
			return;
		}

		let picked = self.resolve(op);
		let mut transaction = self.scene.transaction();

		if run(&mut transaction, op, picked).is_err() {
			return;
		}

		let commit = transaction.commit();
		self.binding
			.apply_local(&mut self.scene, &commit, Track::Step)
			.expect("mirror local commit");
	}

	fn resolve(&self, op: Op) -> (NodeId, NodeId) {
		match op {
			Op::Insert { parent, .. } => (self.pick(parent), NodeId::ROOT),
			Op::Move { node, parent, .. } => (self.pick(node), self.pick(parent)),
			Op::Remove { node } | Op::Restyle { node, .. } | Op::Type { node, .. } => {
				(self.pick(node), NodeId::ROOT)
			}
			Op::Undo => (NodeId::ROOT, NodeId::ROOT),
		}
	}
}

fn run(
	transaction: &mut Transaction<'_>,
	op: Op,
	(first, second): (NodeId, NodeId),
) -> Result<(), canvist_scene::SceneError> {
	match op {
		Op::Insert { kind, .. } => {
			transaction.insert(
				first,
				None,
				KINDS[usize::from(kind) % KINDS.len()],
				Props::default(),
			)?;
		}
		Op::Remove { .. } => transaction.remove(&[first])?,
		Op::Move { front, .. } => transaction.move_to(first, second, front.then_some(0))?,
		Op::Restyle { value, .. } => {
			let amount = f64::from(value);
			transaction.set(first, Prop::Opacity(amount / 255.0))?;
			transaction.set(first, Prop::Transform(Affine::translate((amount, 0.0))))?;
		}
		Op::Type { at, .. } => {
			transaction.text(first, |inner| {
				// A peer may have written an empty frame; there is nowhere to type.
				let Some(block) = inner.doc().first_text_block() else {
					return Ok(None);
				};
				let len = inner
					.doc()
					.inline(block)
					.map_or(0, canvist_model::InlineText::len);
				// Only ASCII is typed, so every offset is a character boundary.
				let offset = u32::from(at) % (len + 1);
				inner
					.insert_text(Point::new(block, offset), "ab", MarkSet::empty())
					.map(Some)
			})?;
		}
		Op::Undo => {}
	}

	Ok(())
}

/// Messages in flight: (sender, update).
#[derive(Default)]
struct Network {
	queue: Vec<(usize, Vec<u8>)>,
}

impl Network {
	fn broadcast(&mut self, from: usize, peers: &mut [Peer]) {
		for update in peers[from].binding.take_updates() {
			self.queue.push((from, update));
		}
	}

	fn deliver(&mut self, choice: u32, twice: bool, peers: &mut [Peer]) {
		if self.queue.is_empty() {
			return;
		}

		let index = choice as usize % self.queue.len();
		let (from, update) = if twice {
			self.queue[index].clone()
		} else {
			self.queue.swap_remove(index)
		};

		for to in (0..peers.len()).filter(|&to| to != from) {
			let peer = &mut peers[to];
			peer.binding
				.apply_update(&mut peer.scene, &update, Encoding::V1, None)
				.expect("apply update");
			self.broadcast(to, peers);
		}
	}

	fn drain(&mut self, peers: &mut [Peer]) {
		while !self.queue.is_empty() {
			self.deliver(0, false, peers);
		}
	}
}

fn session(events: &[Event]) -> Result<(), TestCaseError> {
	let schema = schema();
	let scene = Scene::new(schema.clone(), 1).expect("scene");
	let mut first = SceneBinding::with_client_id(schema.clone(), 1).expect("binding");
	first.seed(&scene).expect("seed");
	let state = first.encode_state(Encoding::V1);
	let mut peers = vec![Peer {
		scene,
		binding: first,
	}];

	for client in 2..=PEERS {
		let client = u32::try_from(client).expect("few peers");
		let mut binding =
			SceneBinding::with_client_id(schema.clone(), u64::from(client)).expect("binding");
		binding.apply_state(&state, Encoding::V1).expect("state");
		let scene = binding.scene(client).expect("join");
		peers.push(Peer { scene, binding });
	}

	let mut network = Network::default();

	for event in events {
		match *event {
			Event::Edit(peer, op) => {
				peers[peer].apply(op);
				network.broadcast(peer, &mut peers);
			}
			Event::Deliver(choice, twice) => network.deliver(choice, twice, &mut peers),
		}
	}

	network.drain(&mut peers);
	let expected = peers[0].scene.to_json();

	for peer in &peers {
		prop_assert_eq!(peer.scene.check(), Ok(()));
		prop_assert_eq!(&peer.scene.to_json(), &expected);
		prop_assert!(!peer.binding.is_waiting());
	}

	let mut fresh = SceneBinding::with_client_id(schema, 99).expect("binding");
	fresh
		.apply_state(&peers[0].binding.encode_state(Encoding::V1), Encoding::V1)
		.expect("state");
	let rebuilt = fresh.scene(99).expect("rebuild");
	prop_assert_eq!(rebuilt.to_json(), expected);
	Ok(())
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(128))]

	#[test]
	fn peers_converge(events in prop::collection::vec(event(), 1..60)) {
		session(&events)?;
	}
}
