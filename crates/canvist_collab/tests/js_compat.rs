//! A JavaScript Yjs peer and a Rust peer converge.
//!
//! `tests/js/peer.ts` runs real `yjs` under Deno and knows only the Yjs
//! schema from the crate docs. Both peers edit concurrently (text with
//! UTF-16 surrogate pairs, marks with and without attributes, embeds, block
//! attributes, splits, and a nested block written from JavaScript), exchange
//! updates, and must then render the same ProseMirror JSON.
//!
//! Requires `deno` on the `PATH` (the devenv shell provides it); the first
//! run downloads `yjs` from npm.

mod support;

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;

use canvist_collab::Encoding;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Step;
use serde_json::Value as Json;
use serde_json::json;
use support::Peer;

/// The Deno process playing the JavaScript peer.
struct JsPeer {
	child: Child,
	stdin: ChildStdin,
	stdout: BufReader<ChildStdout>,
}

/// What the JavaScript peer said in reply to one message.
#[derive(Default)]
struct Reply {
	updates: Vec<Vec<u8>>,
	json: Option<Json>,
}

impl JsPeer {
	fn spawn() -> Self {
		let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/js/peer.ts");
		let mut child = Command::new("deno")
			.args(["run", "--quiet", "--no-prompt", "--allow-env", script])
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.expect("deno runs; the devenv shell provides it");
		let stdin = child.stdin.take().expect("piped stdin");
		let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
		Self {
			child,
			stdin,
			stdout,
		}
	}

	/// Send one message and collect the peer's output until it acknowledges
	/// it: the updates its edits produced and, for a dump, its JSON.
	fn send(&mut self, message: &Json) -> Reply {
		writeln!(self.stdin, "{message}").expect("the peer reads stdin");
		let mut reply = Reply::default();

		loop {
			let mut line = String::new();
			let read = self
				.stdout
				.read_line(&mut line)
				.expect("the peer writes stdout");
			assert!(read > 0, "the JavaScript peer exited early");
			let message: Json = serde_json::from_str(&line).expect("one JSON message per line");

			if message.get("done").is_some() {
				return reply;
			}

			if let Some(update) = message.get("update").and_then(Json::as_str) {
				reply.updates.push(from_hex(update));
			}

			if let Some(json) = message.get("json") {
				reply.json = Some(json.clone());
			}
		}
	}

	fn apply(&mut self, update: &[u8]) {
		self.send(&json!({ "update": to_hex(update) }));
	}

	fn edit(&mut self, name: &str) -> Vec<Vec<u8>> {
		self.send(&json!({ "edit": name })).updates
	}

	fn dump(&mut self) -> Json {
		self.send(&json!({ "dump": true }))
			.json
			.expect("a dump replies with JSON")
	}
}

impl Drop for JsPeer {
	fn drop(&mut self) {
		// The script ends when its stdin closes; reap it either way.
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

fn to_hex(bytes: &[u8]) -> String {
	const DIGITS: &[u8; 16] = b"0123456789abcdef";

	bytes
		.iter()
		.flat_map(|byte| [byte >> 4, byte & 0xf])
		.map(|digit| char::from(DIGITS[usize::from(digit)]))
		.collect()
}

fn from_hex(hex: &str) -> Vec<u8> {
	(0..hex.len())
		.step_by(2)
		.map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex"))
		.collect()
}

fn child(peer: &Peer, path: &[usize]) -> BlockId {
	let mut parent = None;

	for &index in path {
		parent = Some(peer.doc.children(parent)[index]);
	}

	parent.expect("a non-empty path")
}

#[test]
fn a_javascript_peer_and_a_rust_peer_converge() {
	let schema = support::schema();
	let mut rust = Peer::seed(support::starter(&schema, 1), 1);
	let mut js = JsPeer::spawn();
	js.apply(&rust.binding.encode_state(Encoding::V1));

	assert_eq!(
		js.dump(),
		rust.json(),
		"JavaScript reads the seeded document"
	);

	// Rust edits while JavaScript edits concurrently.
	let title = child(&rust, &[0]);
	let end = child(&rust, &[5]);
	let one = child(&rust, &[2, 0, 0]);
	rust.edit(|tx| {
		tx.insert_text(Point::new(title, 5), " 2026", MarkSet::empty())
			.is_ok()
	})
	.expect("type in the title");
	let bold = Mark::new(schema.mark_type("bold").expect("bold"));
	rust.step(Step::AddMark {
		block: one,
		range: 0..3,
		mark: bold,
	})
	.expect("bold one");
	let paragraph = schema.default_text_block();
	rust.edit(|tx| {
		tx.split(Point::new(end, 1), paragraph, Attrs::new())
			.is_ok()
	})
	.expect("split end");

	let mut from_js = js.edit("concurrent");
	from_js.extend(js.edit("nested"));

	for update in rust.binding.take_updates() {
		js.apply(&update);
	}

	for update in &from_js {
		rust.receive(update);
	}

	assert!(!rust.binding.is_waiting());
	let expected = rust.json();
	assert_eq!(js.dump(), expected, "both peers render the same document");
	assert_eq!(rust.rebuilt(), expected);

	let texts: Vec<_> = rust
		.doc
		.text_blocks()
		.filter_map(|block| block.inline())
		.map(|text| text.as_str().to_owned())
		.collect();
	assert_eq!(
		texts,
		[
			"Title 2026",
			"bold é 🎉",
			"one",
			"two",
			"from js",
			"quoted",
			"e\u{FFFC}",
			"nd",
		]
	);
	assert_eq!(expected["content"][0]["attrs"]["level"], json!(2));
}
