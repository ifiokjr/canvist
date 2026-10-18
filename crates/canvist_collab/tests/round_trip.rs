//! Local steps survive the trip into yrs and back out on another peer.

mod support;

use canvist_collab::Encoding;
use proptest::prelude::*;
use support::Peer;
use support::ops::Op;

fn op() -> impl Strategy<Value = Op> {
	(0_u8..10, any::<u32>(), any::<u32>(), any::<u32>())
		.prop_map(|(kind, a, b, c)| Op { kind, a, b, c })
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(256))]

	/// One peer edits, the other follows every update immediately.
	#[test]
	fn a_follower_mirrors_every_step(ops in prop::collection::vec(op(), 1..40)) {
		let schema = support::schema();
		let mut leader = Peer::seed(support::starter(&schema, 1), 1);
		let state = leader.binding.encode_state(Encoding::V1);
		let mut follower = Peer::join(schema, 2, &state);

		for op in ops {
			if !op.apply(&mut leader) {
				continue;
			}

			for update in leader.binding.take_updates() {
				follower.receive(&update);
			}

			prop_assert_eq!(follower.json(), leader.json(), "after {:?}", op);
		}

		prop_assert_eq!(leader.rebuilt(), leader.json());
	}
}
