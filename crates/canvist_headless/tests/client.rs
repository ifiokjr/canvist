//! Shared `Doc`s as y-sync clients of a server `Doc`, as the browser editor
//! uses them: joining with no state, reporting the commits of remote
//! changes, and undoing only their own changes.

#![cfg(all(feature = "collab", feature = "starter-kit"))]

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use canvist_headless::canvist_editor::KeyPress;
use canvist_headless::canvist_editor::Modifiers;
use canvist_headless::join_message;
use serde_json::Value as Json;

/// A server holding `markdown`, shared as yrs client 1.
fn server(engine: &Engine<canvist_extensions::StarterKitHandles>, markdown: &str) -> Doc {
	let mut doc = engine
		.parse(Format::Markdown, markdown)
		.expect("valid markdown");
	doc.share(1).expect("not shared yet");
	doc
}

/// A client joined to `server` as yrs client `id`, with what it sent the
/// server delivered.
fn join(engine: &Engine<canvist_extensions::StarterKitHandles>, server: &mut Doc, id: u64) -> Doc {
	let answer = server.receive_sync(&join_message()).expect("valid step 1");
	let mut client = None;

	for message in answer.reply {
		if let Some((doc, _)) = engine.join_sync(&message, id).expect("valid reply") {
			client = Some(doc);
		}
	}

	let mut client = client.expect("the server answers with sync step 2");
	deliver(&mut client, server);
	client
}

/// Send `from`'s pending changes to `to`.
fn deliver(from: &mut Doc, to: &mut Doc) {
	for message in from.flush_sync().expect("shared") {
		to.receive_sync(&message).expect("valid update");
	}
}

fn text(doc: &Doc) -> String {
	doc.export(Format::Markdown).trim().to_owned()
}

fn type_at_end_of_first(doc: &mut Doc, typed: &str) -> Result<(), HeadlessError> {
	let caret = Position::new([0], 5);
	doc.select_range(&caret, &caret)?;
	doc.insert_text(typed)?;
	Ok(())
}

#[test]
fn a_client_joins_from_sync_step_2_alone() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut server = server(&engine, "Hello");

	// The server's own step 1 carries no state: a joining client waits.
	assert!(engine.join_sync(&server.sync_start()?, 2)?.is_none());

	let client = join(&engine, &mut server, 2);
	assert_eq!(text(&client), "Hello");
	assert!(client.is_shared());
	Ok(())
}

/// Alice's pending changes through the server: what it broadcasts to every
/// other connection.
fn alice_broadcast(alice: &mut Doc, server: &mut Doc) -> Result<Vec<Vec<u8>>, HeadlessError> {
	let mut broadcast = Vec::new();

	for message in alice.flush_sync()? {
		broadcast.extend(server.receive_sync(&message)?.broadcast);
	}

	Ok(broadcast)
}

#[test]
fn a_relayed_keystroke_is_one_commit_for_the_receiver() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut server = server(&engine, "Hello");
	let mut alice = join(&engine, &mut server, 2);
	let mut bob = join(&engine, &mut server, 3);

	type_at_end_of_first(&mut alice, "?")?;
	let broadcast = alice_broadcast(&mut alice, &mut server)?;
	let mut commits = Vec::new();

	for message in &broadcast {
		commits.extend(bob.receive_sync_changes(message)?.1);
	}

	assert_eq!(text(&bob), "Hello?");
	assert_eq!(commits.len(), 1);
	assert!(!commits[0].is_empty());
	Ok(())
}

#[test]
fn undo_keys_and_commands_undo_only_this_peers_changes() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut server = server(&engine, "Hello");
	let mut alice = join(&engine, &mut server, 2);
	let mut bob = join(&engine, &mut server, 3);

	type_at_end_of_first(&mut alice, "!")?;
	for message in alice_broadcast(&mut alice, &mut server)? {
		bob.receive_sync(&message)?;
	}

	let caret = Position::new([0], 0);
	bob.select_range(&caret, &caret)?;
	bob.insert_text("Oh, ")?;
	assert_eq!(text(&bob), "Oh, Hello!");

	let mod_z = KeyPress::new(
		"z",
		Modifiers {
			ctrl: true,
			..Modifiers::NONE
		},
	);
	assert_eq!(bob.shared_history_key(mod_z), Some(false));
	assert!(bob.press("Mod-z")?);
	assert_eq!(text(&bob), "Hello!", "Alice's `!` stays");

	assert!(bob.run_named("redo", &Json::Null)?);
	assert_eq!(text(&bob), "Oh, Hello!");
	assert!(bob.run_named("undo", &Json::Null)?);
	assert_eq!(text(&bob), "Hello!");

	let unshared = engine.parse(Format::Markdown, "Local")?;
	assert_eq!(unshared.shared_history_key(mod_z), None);
	Ok(())
}

#[test]
fn typing_a_word_is_one_shared_undo_step_despite_echoes() -> Result<(), HeadlessError> {
	use std::time::Duration;

	use canvist_headless::canvist_editor::ParagraphNavigator;

	let engine = Engine::starter_kit();
	let mut server = server(&engine, "");
	let mut alice = join(&engine, &mut server, 2);
	let mut lines = ParagraphNavigator;

	for (index, typed) in ["W", "o", "r", "d"].into_iter().enumerate() {
		let now = Duration::from_millis(1_000 + 20 * index as u64);
		alice.edit(now, &mut lines, |editor, env| {
			editor.insert_text(typed, env)
		})?;

		// y-websocket's server echoes each update to its sender.
		for echo in alice.flush_sync()? {
			alice.receive_sync(&echo)?;
		}
	}

	assert_eq!(text(&alice), "Word");
	alice.undo()?;
	assert_eq!(text(&alice), "");
	Ok(())
}
