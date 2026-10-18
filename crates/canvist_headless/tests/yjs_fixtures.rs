//! Generates the Yjs payloads embedded in `conformance/fixtures/collab.json`.
//!
//! Client ids are fixed, so the output is the same on every run and every
//! binding reproduces the states the payloads build on. Regenerate with:
//!
//! ```sh
//! cargo test -p canvist_headless --features collab --test yjs_fixtures -- --ignored --nocapture
//! ```

#![cfg(feature = "collab")]

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use canvist_collab::keys;
use canvist_collab::yrs;
use canvist_collab::yrs::Array;
use canvist_collab::yrs::Map;
use canvist_collab::yrs::Transact;
use canvist_collab::yrs::sync::Message;
use canvist_collab::yrs::sync::SyncMessage;
use canvist_collab::yrs::updates::decoder::Decode;
use canvist_collab::yrs::updates::encoder::Encode;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;

/// The yrs client of the peer whose state `joinState` holds.
const ORIGIN_CLIENT: u64 = 101;
/// The yrs client the fixtures share the server document as.
const SERVER_CLIENT: u64 = 10;
/// The block-id client and yrs client of the editing peer.
const PEER_CLIENT: u32 = 20;
/// The yrs client of the raw Yjs peer that writes a block canvist rejects.
const FOREIGN_CLIENT: u64 = 77;

fn print(name: &str, bytes: &[u8]) {
	println!("{name}: {}", STANDARD.encode(bytes));
}

#[test]
#[ignore = "prints fixture payloads; run by hand when the fixtures change"]
fn print_yjs_fixture_payloads() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut origin = engine.parse(Format::Markdown, "# Shared\n\n- one\n- two")?;
	origin.share(ORIGIN_CLIENT)?;
	print("joinState", &origin.encode_state()?);

	// The fixture steps recreate this state exactly: "Hello", shared as
	// `SERVER_CLIENT`. Every update below builds on it.
	let mut server = engine.parse(Format::Markdown, "Hello")?;
	server.share(SERVER_CLIENT)?;
	let state = server.encode_state()?;

	let [typed, bolded] = peer_updates(&state)?;
	print("typeWorld", &typed);
	print("boldWorld", &bolded);
	print("calloutBlock", &rejected_block(&state));
	print(
		"syncStep1Empty",
		&Message::Sync(SyncMessage::SyncStep1(yrs::StateVector::default())).encode_v1(),
	);
	print(
		"syncUpdateTypeWorld",
		&Message::Sync(SyncMessage::Update(typed)).encode_v1(),
	);
	Ok(())
}

/// A peer typing " world" after "Hello", then bolding it: one update each.
fn peer_updates(state: &[u8]) -> Result<[Vec<u8>; 2], HeadlessError> {
	let mut peer = Engine::starter_kit()
		.with_client(PEER_CLIENT)
		.from_yjs(state, u64::from(PEER_CLIENT))?;
	peer.select_range(&Position::new([0], 5), &Position::new([0], 5))?;
	peer.insert_text(" world")?;
	let typed = peer.take_updates()?;

	peer.select_range(&Position::new([0], 6), &Position::new([0], 11))?;
	peer.run_named("toggleBold", &serde_json::Value::Null)?;
	let bolded = peer.take_updates()?;

	match (
		<[Vec<u8>; 1]>::try_from(typed),
		<[Vec<u8>; 1]>::try_from(bolded),
	) {
		(Ok([typed]), Ok([bolded])) => Ok([typed, bolded]),
		_ => panic!("each edit is one update"),
	}
}

/// A raw Yjs peer inserting a `callout` block, a type the StarterKit lacks.
fn rejected_block(state: &[u8]) -> Vec<u8> {
	let foreign = yrs::Doc::with_options(yrs::Options {
		offset_kind: yrs::OffsetKind::Utf16,
		..yrs::Options::with_client_id(yrs::block::ClientID::new(FOREIGN_CLIENT))
	});
	let root = foreign.get_or_insert_array(keys::ROOT);
	let update = yrs::Update::decode_v1(state).expect("the state decodes");
	foreign
		.transact_mut()
		.apply_update(update)
		.expect("the state applies");

	let mut txn = foreign.transact_mut();
	let callout: yrs::MapRef = root.insert(&mut txn, 1, yrs::MapPrelim::default());
	callout.insert(&mut txn, keys::ID, "1z.0");
	callout.insert(&mut txn, keys::TYPE, "callout");
	callout.insert(&mut txn, keys::TEXT, yrs::TextPrelim::new("note"));
	txn.encode_update_v1()
}
