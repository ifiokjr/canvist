//! A Rust `SyncSession` and a JavaScript `HocuspocusProvider` collaborate
//! through a real Hocuspocus server.
//!
//! `tests/js/hocuspocus.ts` runs `@hocuspocus/server` under Deno with a
//! stock provider connected over a WebSocket, and relays the Rust session's
//! frames to the server. The Rust peer seeds the document; both peers then
//! edit concurrently, publish carets through awareness, and must agree.

mod support;

use canvist_collab::Binding;
use canvist_collab::Framing;
use canvist_collab::Presence;
use canvist_collab::SyncSession;
use canvist_collab::Track;
use canvist_collab::User;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use serde_json::Value as Json;
use serde_json::json;
use support::deno::Script;
use support::deno::to_hex;

/// The Rust peer and the script running the server and JavaScript client.
struct Harness {
	server: Script,
	session: SyncSession,
	doc: Document,
}

impl Harness {
	/// Send a message to the script, handle every frame the server sent back
	/// for the Rust session, and keep exchanging until both sides are quiet.
	/// Returns the script's other replies.
	fn send(&mut self, message: &Json) -> Vec<Json> {
		let replies = self.server.send(message);
		let frames = Script::binary(&replies, "frame");
		let mut outgoing = self.receive(&frames);
		outgoing.extend(self.session.flush().expect("flush"));

		while !outgoing.is_empty() {
			let mut next = Vec::new();

			for frame in outgoing {
				let replies = self.server.send(&json!({ "frame": to_hex(&frame) }));
				next.extend(self.receive(&Script::binary(&replies, "frame")));
			}

			next.extend(self.session.flush().expect("flush"));
			outgoing = next;
		}

		replies
			.into_iter()
			.filter(|reply| reply.get("frame").is_none())
			.collect()
	}

	/// Handle frames from the server, returning the session's replies.
	fn receive(&mut self, frames: &[Vec<u8>]) -> Vec<Vec<u8>> {
		frames
			.iter()
			.flat_map(|frame| {
				self.session
					.receive(&mut self.doc, frame)
					.expect("a frame the session understands")
					.replies
			})
			.collect()
	}

	/// Send the session's own frames, as when it opens or flushes.
	fn deliver(&mut self, frames: Vec<Vec<u8>>) {
		for frame in frames {
			self.send(&json!({ "frame": to_hex(&frame) }));
		}
	}

	fn dump(&mut self) -> Json {
		self.send(&json!({ "dump": true }))
			.into_iter()
			.find(|reply| reply.get("json").is_some())
			.expect("a dump replies")
	}
}

#[test]
fn rust_and_javascript_collaborate_through_hocuspocus() {
	let schema = support::schema();
	let doc = support::starter(&schema, 1);
	let mut binding = Binding::with_client_id(schema, 1).expect("binding");
	binding.seed(&doc).expect("seed");
	let presence = Presence::new(&binding);
	let framing = Framing::Hocuspocus {
		document: "canvist".into(),
		token: "".into(),
	};
	let session = SyncSession::new(binding, presence, framing);
	let mut peer = Harness {
		server: Script::spawn("hocuspocus.ts"),
		session,
		doc,
	};

	let opening = peer.session.open().expect("open");
	peer.deliver(opening);
	assert!(peer.session.is_synced(), "the server answered sync step 1");
	assert_eq!(
		peer.dump()["json"],
		peer.doc.to_json(),
		"JavaScript has the seed"
	);

	// Concurrent edits on both sides, then let the server merge them.
	let commits = support::concurrent_edits(&mut peer.doc);

	for commit in &commits {
		peer.session
			.binding_mut()
			.apply_local(&mut peer.doc, commit, Track::Step)
			.expect("local");
	}

	peer.send(&json!({ "edit": "concurrent" }));
	peer.send(&json!({ "edit": "nested" }));
	peer.send(&json!({ "pump": true }));

	assert_eq!(support::texts(&peer.doc), support::MERGED);
	assert_eq!(peer.dump()["json"], peer.doc.to_json(), "both peers agree");

	// Carets travel through the server both ways.
	peer.send(&json!({ "cursor": { "path": [0], "index": 3 } }));
	let title = peer.doc.children(None)[0];
	let seen = peer.session.collaborators(&peer.doc);
	let js = seen
		.iter()
		.find(|collaborator| {
			collaborator
				.user
				.as_ref()
				.is_some_and(|user| user.name == "js")
		})
		.expect("the JavaScript caret arrives");
	assert_eq!(js.selection, Some(Selection::caret(Point::new(title, 3))));

	let user = User {
		name: "rust".to_owned(),
		color: "#f97316".to_owned(),
	};
	let bold = peer.doc.children(None)[1];
	let caret = Selection::caret(Point::new(bold, 7));
	peer.session.set_presence(&peer.doc, &user, Some(caret));
	peer.send(&json!({ "pump": true }));
	assert_eq!(
		peer.dump()["cursors"],
		json!([{ "name": "rust", "block": bold.to_string(), "index": 6 }]),
		"byte offset 7, after `bold é`, is UTF-16 index 6"
	);
}
