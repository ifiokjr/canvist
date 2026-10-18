//! A JavaScript Yjs peer and a Rust peer converge.
//!
//! `tests/js/peer.ts` runs real `yjs` under Deno and knows only the Yjs
//! schema from the crate docs. Both peers edit concurrently (text with
//! UTF-16 surrogate pairs, marks with and without attributes, embeds, block
//! attributes, splits, and a nested block written from JavaScript), exchange
//! updates, and must then render the same ProseMirror JSON.

mod support;

use canvist_collab::Encoding;
use serde_json::json;
use support::Peer;
use support::deno::Script;
use support::deno::to_hex;

#[test]
fn a_javascript_peer_and_a_rust_peer_converge() {
	let schema = support::schema();
	let mut rust = Peer::seed(support::starter(&schema, 1), 1);
	let mut js = Script::spawn("peer.ts");
	let state = rust.binding.encode_state(Encoding::V1);
	js.send(&json!({ "update": to_hex(&state) }));
	assert_eq!(
		dump(&mut js),
		rust.json(),
		"JavaScript reads the seeded document"
	);

	for commit in support::concurrent_edits(&mut rust.doc) {
		rust.record(&commit);
	}

	let mut from_js = Script::binary(&js.send(&json!({ "edit": "concurrent" })), "update");
	from_js.extend(Script::binary(
		&js.send(&json!({ "edit": "nested" })),
		"update",
	));

	for update in rust.binding.take_updates() {
		js.send(&json!({ "update": to_hex(&update) }));
	}

	for update in &from_js {
		rust.receive(update);
	}

	assert!(!rust.binding.is_waiting());
	let expected = rust.json();
	assert_eq!(
		dump(&mut js),
		expected,
		"both peers render the same document"
	);
	assert_eq!(rust.rebuilt(), expected);
	assert_eq!(support::texts(&rust.doc), support::MERGED);
	assert_eq!(expected["content"][0]["attrs"]["level"], json!(2));
}

fn dump(js: &mut Script) -> serde_json::Value {
	js.send(&json!({ "dump": true }))
		.into_iter()
		.find_map(|reply| reply.get("json").cloned())
		.expect("a dump replies with JSON")
}
