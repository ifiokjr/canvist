//! Peers editing concurrently converge on the same document.
//!
//! Each case starts several peers from one seeded document, lets them apply
//! random steps while updates are delivered in random order (some twice),
//! then delivers everything. Every peer's model must then equal every other
//! peer's, and equal a model rebuilt from scratch out of its shared document.

mod support;

use proptest::prelude::*;
use support::Network;
use support::Peer;
use support::ops::Op;

/// One thing that happens during a session.
#[derive(Clone, Copy, Debug)]
enum Event {
	/// A peer edits.
	Edit(u8, Op),
	/// A queued message arrives, possibly not for the last time.
	Deliver(u32, bool),
}

fn op() -> impl Strategy<Value = Op> {
	(any::<u8>(), any::<u32>(), any::<u32>(), any::<u32>())
		.prop_map(|(kind, a, b, c)| Op { kind, a, b, c })
}

fn event(peers: u8) -> impl Strategy<Value = Event> {
	prop_oneof![
		3 => (0..peers, op()).prop_map(|(peer, op)| Event::Edit(peer, op)),
		2 => (any::<u32>(), prop::bool::weighted(0.2)).prop_map(|(choice, twice)| Event::Deliver(choice, twice)),
	]
}

fn session(peer_count: usize, events: &[Event]) -> Result<(), TestCaseError> {
	let schema = support::schema();
	let first = Peer::seed(support::starter(&schema, 1), 1);
	let state = first.binding.encode_state(canvist_collab::Encoding::V1);
	let mut peers = vec![first];

	for client in 2..=peer_count {
		let client = u32::try_from(client).expect("few peers");
		peers.push(Peer::join(schema.clone(), client, &state));
	}

	let mut network = Network::default();

	for event in events {
		match *event {
			Event::Edit(peer, op) => {
				let peer = usize::from(peer);
				op.apply(&mut peers[peer]);
				network.broadcast(peer, &mut peers);
			}
			Event::Deliver(choice, twice) => network.deliver(choice, twice, &mut peers),
		}
	}

	network.drain(&mut peers);
	let expected = peers[0].json();

	for (index, peer) in peers.iter().enumerate() {
		support::assert_invariants(&peer.doc);
		prop_assert_eq!(&peer.json(), &expected, "peer {} diverged", index);
		prop_assert_eq!(
			&peer.rebuilt(),
			&expected,
			"peer {} differs from its shared state",
			index
		);
	}

	Ok(())
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(128))]

	#[test]
	fn two_peers_converge(events in prop::collection::vec(event(2), 1..60)) {
		session(2, &events)?;
	}

	#[test]
	fn many_peers_converge(events in prop::collection::vec(event(4), 1..80)) {
		session(4, &events)?;
	}
}
