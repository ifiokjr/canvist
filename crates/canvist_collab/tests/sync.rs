//! Sync sessions talking the y-sync protocol through a relay server.
//!
//! The server is plain yrs: its own document, awareness, and the stock
//! `DefaultProtocol`, which is what y-sync servers built on yrs run. It
//! answers each client and relays updates to the others, as y-websocket's
//! server does.

mod support;

use canvist_collab::Binding;
use canvist_collab::Framing;
use canvist_collab::History;
use canvist_collab::Presence;
use canvist_collab::SyncSession;
use canvist_collab::User;
use canvist_collab::yrs;
use canvist_model::Document;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Selection;
use yrs::sync::Awareness;
use yrs::sync::DefaultProtocol;
use yrs::sync::Protocol;
use yrs::updates::encoder::Encode;

/// A y-sync relay: a yrs document plus awareness, answering clients with
/// `DefaultProtocol` and forwarding everything they send to the others.
struct Server {
	awareness: Awareness,
	/// Messages waiting for each client.
	outboxes: Vec<Vec<Vec<u8>>>,
}

impl Server {
	fn new(clients: usize) -> Self {
		Self {
			awareness: Awareness::new(yrs::Doc::with_client_id(1_000)),
			outboxes: vec![Vec::new(); clients],
		}
	}

	fn receive(&mut self, from: usize, message: &[u8]) {
		let replies = DefaultProtocol
			.handle(&mut self.awareness, message)
			.expect("the server understands the client");

		for reply in replies {
			self.outboxes[from].push(reply.encode_v1());
		}

		for (to, outbox) in self.outboxes.iter_mut().enumerate() {
			if to != from && !is_sync_step(message) {
				outbox.push(message.to_vec());
			}
		}
	}
}

/// Whether `message` is a sync step, which only concerns its sender.
fn is_sync_step(message: &[u8]) -> bool {
	matches!(message, [0, 0 | 1, ..])
}

/// A client: a document and its session.
struct Client {
	doc: Document,
	session: SyncSession,
}

impl Client {
	fn new(doc: Document, binding: Binding) -> Self {
		let presence = Presence::new(&binding);
		Self {
			doc,
			session: SyncSession::new(binding, presence, Framing::YWebsocket),
		}
	}

	fn deliver(&mut self, messages: Vec<Vec<u8>>, server: &mut Server, index: usize) {
		for message in messages {
			let received = self
				.session
				.receive(&mut self.doc, &message)
				.expect("the client understands the server");

			for reply in received.replies {
				server.receive(index, &reply);
			}
		}
	}
}

/// Move messages until nobody has anything left to say.
fn settle(server: &mut Server, clients: &mut [Client]) {
	loop {
		let mut quiet = true;

		for (index, client) in clients.iter_mut().enumerate() {
			for message in client.session.flush().expect("flush") {
				quiet = false;
				server.receive(index, &message);
			}
		}

		for (index, client) in clients.iter_mut().enumerate() {
			let inbox = std::mem::take(&mut server.outboxes[index]);
			quiet &= inbox.is_empty();
			client.deliver(inbox, server, index);
		}

		if quiet {
			return;
		}
	}
}

fn connect(server: &mut Server, clients: &mut [Client]) {
	for (index, client) in clients.iter_mut().enumerate() {
		for message in client.session.open().expect("open") {
			server.receive(index, &message);
		}
	}

	settle(server, clients);
}

fn two_clients() -> (Server, Vec<Client>) {
	let schema = support::schema();
	let doc = support::starter(&schema, 1);
	let mut binding = Binding::with_client_id(schema.clone(), 1).expect("binding");
	binding.seed(&doc).expect("seed");
	let alice = Client::new(doc, binding);
	let bob = Client::new(
		Document::empty(schema.clone(), 2),
		Binding::with_client_id(schema, 2).expect("binding"),
	);
	let mut server = Server::new(2);
	let mut clients = vec![alice, bob];
	connect(&mut server, &mut clients);
	(server, clients)
}

#[test]
fn a_joining_client_syncs_through_the_server() {
	let (_, clients) = two_clients();

	assert!(clients.iter().all(|client| client.session.is_synced()));
	assert_eq!(clients[1].doc.to_json(), clients[0].doc.to_json());
}

#[test]
fn edits_and_undo_reach_the_other_client() {
	let (mut server, mut clients) = two_clients();
	let title = clients[0].doc.children(None)[0];
	let alice = &mut clients[0];
	let mut transaction = alice.doc.transaction();
	transaction
		.insert_text(Point::new(title, 5), "!", MarkSet::empty())
		.expect("type");
	let commit = transaction.commit();
	alice
		.session
		.binding_mut()
		.record(&mut alice.doc, &commit, false)
		.expect("record");
	settle(&mut server, &mut clients);
	assert_eq!(clients[1].doc.to_json(), clients[0].doc.to_json());

	let alice = &mut clients[0];
	History::undo(alice.session.binding_mut(), &mut alice.doc).expect("undo");
	settle(&mut server, &mut clients);
	assert_eq!(
		clients[1].doc.plain_text(),
		support::starter(&support::schema(), 1).plain_text()
	);
}

#[test]
fn carets_follow_the_text_they_sit_in() {
	let (mut server, mut clients) = two_clients();
	let title = clients[0].doc.children(None)[0];
	let ifiok = User {
		name: "Ifiok".to_owned(),
		color: "#f97316".to_owned(),
	};
	let caret = Selection::caret(Point::new(title, 3));
	let alice = &mut clients[0];
	alice.session.set_presence(&alice.doc, &ifiok, Some(caret));
	settle(&mut server, &mut clients);

	let bob = &mut clients[1];
	let seen = bob.session.collaborators(&bob.doc);
	assert_eq!(seen.len(), 1, "bob sees only alice");
	assert_eq!(seen[0].user.as_ref(), Some(&ifiok));
	assert_eq!(seen[0].selection, Some(caret));

	// Bob types before Alice's caret; her caret moves with the text.
	let mut transaction = bob.doc.transaction();
	transaction
		.insert_text(Point::new(title, 0), "😀 ", MarkSet::empty())
		.expect("type");
	let commit = transaction.commit();
	bob.session
		.binding_mut()
		.record(&mut bob.doc, &commit, false)
		.expect("record");

	let moved = Point::new(title, 3 + "😀 ".len() as u32);
	let seen = bob.session.collaborators(&bob.doc);
	assert_eq!(seen[0].selection, Some(Selection::caret(moved)));
}

#[test]
fn a_caret_at_the_end_of_a_block_stays_at_the_end() {
	let (mut server, mut clients) = two_clients();
	let title = clients[0].doc.children(None)[0];
	let user = User {
		name: "Ada".to_owned(),
		color: "#22c55e".to_owned(),
	};
	let end = Selection::caret(Point::new(title, 5));
	let alice = &mut clients[0];
	alice.session.set_presence(&alice.doc, &user, Some(end));
	settle(&mut server, &mut clients);

	let bob = &mut clients[1];
	assert_eq!(bob.session.collaborators(&bob.doc)[0].selection, Some(end));
}
