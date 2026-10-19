//! A shared `Doc` as a y-websocket server for `canvist_collab` clients:
//! clients join through it, their edits reach each other through it, and
//! the server's own edits reach them.

#![cfg(all(feature = "collab", feature = "starter-kit"))]

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use canvist_headless::canvist_collab::Binding;
use canvist_headless::canvist_collab::Framing;
use canvist_headless::canvist_collab::History;
use canvist_headless::canvist_collab::Presence;
use canvist_headless::canvist_collab::SyncSession;
use canvist_headless::canvist_collab::keys;
use canvist_headless::canvist_collab::yrs;
use canvist_headless::canvist_collab::yrs::Array;
use canvist_headless::canvist_collab::yrs::Transact;
use canvist_headless::canvist_collab::yrs::updates::decoder::Decode;
use canvist_headless::canvist_model::Document;
use canvist_headless::canvist_model::MarkSet;
use canvist_headless::canvist_model::Point;

/// A client: a model document and its y-sync session.
struct Client {
	doc: Document,
	session: SyncSession,
	inbox: Vec<Vec<u8>>,
}

impl Client {
	fn join(engine: &Engine<canvist_extensions::StarterKitHandles>, id: u32) -> Self {
		let binding =
			Binding::with_client_id(engine.schema().clone(), u64::from(id)).expect("binding");
		let presence = Presence::new(&binding);

		Self {
			doc: Document::empty(engine.schema().clone(), id),
			session: SyncSession::new(binding, presence, Framing::YWebsocket),
			inbox: Vec::new(),
		}
	}

	/// Type `text` at the start of the first block and record it for
	/// collaborators.
	fn type_at_start(&mut self, text: &str) {
		let block = self.doc.first_text_block().expect("a text block");
		let mut transaction = self.doc.transaction();
		transaction
			.insert_text(Point::new(block, 0), text, MarkSet::empty())
			.expect("type");
		let commit = transaction.commit();
		self.session
			.binding_mut()
			.record(&mut self.doc, &commit, false)
			.expect("record");
	}
}

/// The server: one shared document and the connections to it.
struct Server {
	doc: Doc,
	clients: Vec<Client>,
}

impl Server {
	/// Send `message` from client `from` to the server and route the replies.
	fn receive(&mut self, from: usize, message: &[u8]) -> Result<(), HeadlessError> {
		let sync = self.doc.receive_sync(message)?;
		self.clients[from].inbox.extend(sync.reply);

		for (index, client) in self.clients.iter_mut().enumerate() {
			if index != from {
				client.inbox.extend(sync.broadcast.iter().cloned());
			}
		}

		Ok(())
	}

	fn connect(&mut self, client: Client) -> Result<(), HeadlessError> {
		let index = self.clients.len();
		self.clients.push(client);
		let greeting = self.doc.sync_start()?;
		self.clients[index].inbox.push(greeting);

		for message in self.clients[index].session.open()? {
			self.receive(index, &message)?;
		}

		self.settle()
	}

	/// Deliver messages until nobody has anything left to say.
	fn settle(&mut self) -> Result<(), HeadlessError> {
		loop {
			let mut moved = self.broadcast_server_changes()?;

			for index in 0..self.clients.len() {
				moved |= self.exchange(index)?;
			}

			if !moved {
				return Ok(());
			}
		}
	}

	/// Queue the server's own changes for every client.
	fn broadcast_server_changes(&mut self) -> Result<bool, HeadlessError> {
		let messages = self.doc.flush_sync()?;

		for client in &mut self.clients {
			client.inbox.extend(messages.iter().cloned());
		}

		Ok(!messages.is_empty())
	}

	/// Deliver client `index`'s inbox, then send the server everything it
	/// has to say. Returns whether any message moved.
	fn exchange(&mut self, index: usize) -> Result<bool, HeadlessError> {
		let client = &mut self.clients[index];
		let inbox = std::mem::take(&mut client.inbox);
		let mut outgoing = client.session.flush()?;

		for message in &inbox {
			outgoing.extend(client.session.receive(&mut client.doc, message)?.replies);
		}

		for message in &outgoing {
			self.receive(index, message)?;
		}

		Ok(!(inbox.is_empty() && outgoing.is_empty()))
	}

	fn assert_converged(&self) {
		for client in &self.clients {
			assert_eq!(client.doc.to_json(), self.doc.to_json());
		}
	}
}

fn server(
	markdown: &str,
) -> Result<(Engine<canvist_extensions::StarterKitHandles>, Server), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut doc = engine.parse(Format::Markdown, markdown)?;
	doc.share(1_000)?;

	let mut server = Server {
		doc,
		clients: Vec::new(),
	};
	server.connect(Client::join(&engine, 1))?;
	server.connect(Client::join(&engine, 2))?;
	Ok((engine, server))
}

#[test]
fn clients_join_and_edit_through_the_server() -> Result<(), HeadlessError> {
	let (_, mut server) = server("# Shared\n\nBody")?;
	server.assert_converged();

	server.clients[0].type_at_start("Alice: ");
	server.clients[1].type_at_start("Bob: ");
	server.settle()?;

	server.assert_converged();
	let title = server.doc.export(Format::Text);
	assert!(
		title.contains("Alice: ") && title.contains("Bob: "),
		"{title}"
	);
	Ok(())
}

#[test]
fn the_servers_own_commands_reach_every_client() -> Result<(), HeadlessError> {
	let (engine, mut server) = server("Body")?;
	let kit = engine.handles();

	server.doc.select_all()?;
	server.doc.run(&kit.bold.toggle())?;
	server.settle()?;

	server.assert_converged();
	assert_eq!(server.doc.export(Format::Markdown), "**Body**");

	// Shared undo reverts only the server's own change.
	server.clients[0].type_at_start("x");
	server.settle()?;
	assert!(server.doc.undo()?);
	server.settle()?;

	server.assert_converged();
	assert_eq!(server.doc.export(Format::Markdown), "xBody");
	Ok(())
}

#[test]
fn a_peer_deleting_every_block_leaves_one_paragraph_everywhere() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut doc = engine.parse(Format::Markdown, "one\n\ntwo")?;
	doc.share(1_000)?;

	let foreign = yrs::Doc::with_client_id(77);
	let root = foreign.get_or_insert_array(keys::ROOT);
	let state = yrs::Update::decode_v1(&doc.encode_state()?).expect("decodes");
	foreign.transact_mut().apply_update(state).expect("applies");
	let deletion = {
		let mut txn = foreign.transact_mut();
		let length = root.len(&txn);
		root.remove_range(&mut txn, 0, length);
		txn.encode_update_v1()
	};

	assert!(doc.apply_update(&deletion)?);
	assert_eq!(doc.to_json()["content"].as_array().map(Vec::len), Some(1));
	assert!(doc.validate()?.is_valid());

	// The paragraph the server added reaches the peer, so both agree.
	let mut peer = engine.with_client(9).from_yjs(&doc.encode_state()?, 9)?;
	assert_eq!(peer.to_json(), doc.to_json());
	assert!(
		peer.take_updates()?.is_empty(),
		"the joining peer needed no block of its own"
	);

	peer.select_range(&Position::new([0], 0), &Position::new([0], 0))?;
	peer.insert_text("again")?;

	for update in peer.take_updates()? {
		doc.apply_update(&update)?;
	}

	assert_eq!(doc.export(Format::Text), "again");
	Ok(())
}
