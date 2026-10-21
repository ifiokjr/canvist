//! Two editors collaborating through a y-sync server, as the example app
//! does with y-websocket. The server is a headless shared document, which
//! answers and relays messages the way y-websocket does.

#![cfg(feature = "collab")]

mod support;

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_layout::DisplayItem;
use support::Driver;
use support::NONE;

/// A room: the server's copy of the document.
struct Room {
	server: Doc,
}

impl Room {
	fn new(markdown: &str) -> Self {
		let mut server = Engine::starter_kit()
			.parse(Format::Markdown, markdown)
			.expect("markdown");
		server.share(1_000).expect("share");
		Self { server }
	}

	/// Deliver `messages` from one client to the server, returning what to
	/// send back to that client and what to relay to the others.
	fn deliver(&mut self, messages: Vec<Vec<u8>>) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
		let mut reply = Vec::new();
		let mut broadcast = Vec::new();

		for message in messages {
			let sync = self.server.receive_sync(&message).expect("sync");
			reply.extend(sync.reply);
			broadcast.extend(sync.broadcast);
		}

		(reply, broadcast)
	}
}

/// A client: connect, and hand everything back and forth until quiet.
fn join(room: &mut Room, client: u64) -> Driver {
	let mut driver = Driver::new("");
	let request = driver.editor.connect(client);
	let (reply, _) = room.deliver(vec![request]);
	let mut clients = [driver];
	pump(room, &mut clients, 0, reply);
	let [mut driver] = clients;
	driver.frame();
	driver
}

/// Send client `from`'s local changes through the room to the others.
fn flush(room: &mut Room, clients: &mut [Driver], from: usize) {
	let updates = clients[from].editor.flush_sync().expect("flush");
	let (_, broadcast) = room.deliver(updates);

	for other in (0..clients.len()).filter(|&other| other != from) {
		pump(room, clients, other, broadcast.clone());
	}

	for client in clients.iter_mut() {
		client.frame();
	}
}

/// Give `messages` to client `index`, then deliver everything it answers
/// to the room and the room's relays to every other client, until no one
/// has anything left to say.
fn pump(room: &mut Room, clients: &mut [Driver], index: usize, messages: Vec<Vec<u8>>) {
	let mut queue: Vec<(usize, Vec<u8>)> = messages
		.into_iter()
		.map(|message| (index, message))
		.collect();

	while let Some((target, message)) = queue.pop() {
		let answers = clients[target]
			.editor
			.receive_sync(&message)
			.expect("receive");
		let (reply, broadcast) = room.deliver(answers);
		queue.extend(reply.into_iter().map(|message| (target, message)));

		for other in (0..clients.len()).filter(|&other| other != target) {
			queue.extend(broadcast.iter().cloned().map(|message| (other, message)));
		}
	}
}

/// The text the editor shows, from its display list.
fn painted(driver: &Driver) -> String {
	driver
		.editor
		.display_list()
		.items
		.iter()
		.filter_map(|item| {
			match item {
				DisplayItem::Text(run) => Some(run.text()),
				_ => None,
			}
		})
		.collect()
}

#[test]
fn a_client_joins_with_the_rooms_content() {
	let mut room = Room::new("# Shared\n\nHello");
	let alice = join(&mut room, 1);

	assert!(alice.editor.is_shared());
	assert_eq!(alice.markdown(), "# Shared\n\nHello");
	assert_eq!(painted(&alice), "SharedHello");
}

#[test]
fn typing_reaches_the_other_editor_and_lays_out_there() {
	let mut room = Room::new("Hello");
	let mut clients = [join(&mut room, 1), join(&mut room, 2)];

	let _ = clients[0].press("End", support::CTRL);
	clients[0].type_text(" from Alice");
	flush(&mut room, &mut clients, 0);
	let [_, bob] = &clients;

	assert_eq!(bob.markdown(), "Hello from Alice");
	assert_eq!(painted(bob), "Hello from Alice");
	assert_eq!(room.server.export(Format::Markdown), "Hello from Alice");
}

#[test]
fn concurrent_edits_converge_and_carets_stay_with_their_text() {
	let mut room = Room::new("middle");
	let mut clients = [join(&mut room, 1), join(&mut room, 2)];
	let (alice, bob) = (0, 1);

	let _ = clients[alice].press("Home", support::CTRL);
	let _ = clients[bob].press("End", support::CTRL);
	clients[alice].type_text("start ");
	clients[bob].type_text(" end");
	flush(&mut room, &mut clients, alice);
	flush(&mut room, &mut clients, bob);

	assert_eq!(clients[alice].markdown(), "start middle end");
	assert_eq!(clients[bob].markdown(), clients[alice].markdown());

	// Bob keeps typing where his caret was, after Alice's insert moved it.
	clients[bob].type_text("!");
	assert_eq!(clients[bob].markdown(), "start middle end!");
	let _ = clients[bob].press("Enter", NONE);
	flush(&mut room, &mut clients, bob);
	assert_eq!(clients[alice].markdown(), "start middle end!");
	assert_eq!(
		clients[alice].editor.doc().document().children(None).len(),
		2
	);
	assert_eq!(painted(&clients[alice]), "start middle end!");
}

#[test]
fn messages_before_the_state_wait_for_the_join() {
	let mut room = Room::new("Early");
	let mut driver = Driver::new("");
	let request = driver.editor.connect(7);
	let (reply, _) = room.deliver(vec![request]);
	// The server's own sync step 1 arrives before its answer.
	let step1 = room.server.sync_start().expect("step 1");

	let answers = driver.editor.receive_sync(&step1).expect("held");
	assert!(answers.is_empty());
	assert!(!driver.editor.is_shared());

	let mut answers = Vec::new();

	for message in reply {
		answers.extend(driver.editor.receive_sync(&message).expect("join"));
	}

	assert!(driver.editor.is_shared());
	assert!(!answers.is_empty(), "the held step 1 was answered");
	assert_eq!(driver.markdown(), "Early");
}
