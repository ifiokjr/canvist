//! Performance and capacity budgets for the relay.
//!
//! These tests fail when the relay gets slower or hungrier than the budgets
//! in `docs/src/cloud/self-hosting.md`, which canvist Cloud's prices are
//! built on. They only mean something in release mode:
//!
//! ```sh
//! cargo test --release -p canvist_cloud --test perf_budgets -- --ignored --nocapture
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget. Memory is measured with a
//! counting allocator, so it is the heap the relay really holds, not the
//! process's resident size.

#![cfg(not(debug_assertions))]
#![allow(
	unsafe_code,
	reason = "a counting global allocator wraps the system one"
)]

mod support;

use std::alloc::GlobalAlloc;
use std::alloc::Layout;
use std::alloc::System;
use std::sync::Arc;
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use canvist_cloud::auth::Scope;
use canvist_cloud::names::DocumentKey;
use canvist_cloud::names::DocumentName;
use canvist_cloud::sync::Limits;
use futures_util::SinkExt;
use futures_util::StreamExt;
use support::Fixture;
use tokio_tungstenite::tungstenite::Message;
use yrs::Array;
use yrs::Doc;
use yrs::Map;
use yrs::MapPrelim;
use yrs::Text;
use yrs::TextPrelim;
use yrs::Transact;
use yrs::updates::decoder::Decode;

/// Counts live heap bytes.
struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

// SAFETY: every method forwards to the system allocator with the same
// arguments and only adds bookkeeping.
unsafe impl GlobalAlloc for Counting {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
		// SAFETY: forwarded unchanged.
		unsafe { System.alloc(layout) }
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
		// SAFETY: forwarded unchanged.
		unsafe { System.dealloc(ptr, layout) }
	}

	unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
		LIVE.fetch_add(
			new_size as isize - layout.size() as isize,
			Ordering::Relaxed,
		);
		// SAFETY: forwarded unchanged.
		unsafe { System.realloc(ptr, layout, new_size) }
	}
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live_bytes() -> isize {
	LIVE.load(Ordering::Relaxed)
}

fn slack() -> f64 {
	std::env::var("CANVIST_PERF_SLACK")
		.ok()
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

fn assert_time(name: &str, measured: Duration, budget: Duration) {
	let allowed = budget.mul_f64(slack());
	eprintln!("{name}: {measured:?} (budget {budget:?}, allowed {allowed:?})");
	assert!(
		measured <= allowed,
		"{name} took {measured:?}, over its {allowed:?} budget"
	);
}

fn assert_bytes(name: &str, measured: usize, budget: usize) {
	eprintln!(
		"{name}: {} KiB (budget {} KiB)",
		measured / 1024,
		budget / 1024
	);
	assert!(
		measured <= budget,
		"{name} uses {measured} bytes, over its {budget} budget"
	);
}

const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const SAMPLES: usize = 31;
const PEERS: usize = 100;

/// Keystroke updates writing a document of `paragraphs` paragraphs, one
/// update per character, as an editor sends them.
fn keystrokes(client: u64, paragraphs: usize) -> Vec<Vec<u8>> {
	let doc = Doc::with_client_id(client);
	let blocks = doc.get_or_insert_array("blocks");
	let mut updates = Vec::new();

	for index in 0..paragraphs {
		let text = {
			let mut txn = doc.transact_mut();
			let block = blocks.push_back(&mut txn, MapPrelim::default());
			block.insert(&mut txn, "id", format!("{client}.{index}"));
			block.insert(&mut txn, "type", "paragraph");
			block.insert(&mut txn, "attrs", MapPrelim::default());
			let text = block.insert(&mut txn, "text", TextPrelim::new(""));
			updates.push(txn.encode_update_v1());
			text
		};

		for (offset, character) in PARAGRAPH.char_indices() {
			let mut txn = doc.transact_mut();
			text.insert(&mut txn, offset as u32, &character.to_string());
			updates.push(txn.encode_update_v1());
		}
	}

	updates
}

/// The compacted state of the same document, as the store holds it.
fn snapshot(client: u64, paragraphs: usize) -> Vec<u8> {
	let doc = Doc::new();
	{
		let mut txn = doc.transact_mut();
		for update in keystrokes(client, paragraphs) {
			txn.apply_update(yrs::Update::decode_v1(&update).expect("decodes"))
				.expect("applies");
		}
	}
	let txn = doc.transact();
	yrs::ReadTxn::encode_state_as_update_v1(&txn, &yrs::StateVector::default())
}

fn key(fixture: &Fixture, name: &str) -> DocumentKey {
	DocumentKey {
		project: fixture.project.clone(),
		environment: fixture.environment.clone(),
		name: DocumentName::parse(name).expect("valid"),
	}
}

/// Heap bytes per loaded room after `write` fills each of `documents`
/// rooms with its updates.
async fn memory_per_room(fixture: &Fixture, label: &str, updates: &[Vec<Vec<u8>>]) -> usize {
	let rooms = &fixture.server.app().rooms;
	let limits = Limits {
		max_update_bytes: usize::MAX,
		max_document_bytes: usize::MAX,
	};
	let editor: Arc<str> = Arc::from("perf");
	let keys: Vec<DocumentKey> = (0..updates.len())
		.map(|index| key(fixture, &format!("{label}-{index}")))
		.collect();

	for (key, updates) in keys.iter().zip(updates) {
		let (room, membership) = rooms.join(key).await.expect("loads");
		for update in updates {
			room.apply(None, &editor, update, limits).expect("applies");
		}

		// Persist by hand, before measuring, so the store's own copies are
		// not counted; the fixture's flush loop never runs.
		room.persist(rooms.store(), usize::MAX, false)
			.await
			.expect("persists");
		room.leave(membership.peer);
	}

	// Unload every room: what that frees is what the rooms held.
	let loaded = live_bytes();

	for key in &keys {
		rooms.discard(key);
	}

	(loaded - live_bytes()).max(0) as usize / updates.len()
}

/// Memory per document of `paragraphs` paragraphs, loaded from a snapshot
/// (every document after a restart or eviction) and typed live.
async fn memory_per_document(fixture: &Fixture, label: &str, paragraphs: usize) -> (usize, usize) {
	const DOCUMENTS: usize = 100;
	let snapshots: Vec<Vec<Vec<u8>>> = (0..DOCUMENTS)
		.map(|index| vec![snapshot(index as u64 + 1, paragraphs)])
		.collect();
	let typed: Vec<Vec<Vec<u8>>> = (0..DOCUMENTS / 10)
		.map(|index| keystrokes(index as u64 + 1, paragraphs))
		.collect();
	let loaded = memory_per_room(fixture, &format!("{label}-loaded"), &snapshots).await;
	let live = memory_per_room(fixture, &format!("{label}-live"), &typed).await;
	eprintln!(
		"{label}: {paragraphs} paragraphs, {} KiB stored; {} KiB in memory loaded, {} KiB typed \
		 live",
		snapshots[0][0].len() / 1024,
		loaded / 1024,
		live / 1024,
	);
	(loaded, live)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "release-mode performance budget; run with `test:perf`"]
async fn memory_per_loaded_document() {
	let mut config = support::config();
	config.sync.flush_interval = Duration::from_hours(1);
	config.sync.idle_eviction = Duration::from_hours(1);
	let fixture = Fixture::start_with(config, support::store().await).await;

	// A short note, a typical working document, and a long report, every
	// character typed separately.
	let (small, small_live) = memory_per_document(&fixture, "note", 5).await;
	let (typical, typical_live) = memory_per_document(&fixture, "typical", 50).await;
	let (large, large_live) = memory_per_document(&fixture, "report", 500).await;

	// Measured on Apple silicon: 22, 132, and 1,201 KiB loaded; 30, 142,
	// and 1,231 KiB typed. About 2.4 KiB a block plus 10 KiB a room, of
	// which 8 KiB is the fan-out ring.
	assert_bytes("loaded 5-paragraph document", small, 32 * 1024);
	assert_bytes("loaded 50-paragraph document", typical, 192 * 1024);
	assert_bytes("loaded 500-paragraph document", large, 1792 * 1024);
	assert_bytes("typed 5-paragraph document", small_live, 48 * 1024);
	assert_bytes("typed 50-paragraph document", typical_live, 224 * 1024);
	assert_bytes("typed 500-paragraph document", large_live, 1920 * 1024);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "release-mode performance budget; run with `test:perf`"]
async fn keystrokes_apply_and_persist_quickly() {
	let mut config = support::config();
	config.sync.flush_interval = Duration::from_hours(1);
	let fixture = Fixture::start_with(config, support::store().await).await;
	let rooms = &fixture.server.app().rooms;
	let limits = Limits {
		max_update_bytes: usize::MAX,
		max_document_bytes: usize::MAX,
	};
	let editor: Arc<str> = Arc::from("perf");
	// 500 paragraphs typed a character at a time: 42,000 updates.
	let updates = keystrokes(1, 500);
	let (room, membership) = rooms.join(&key(&fixture, "typing")).await.expect("loads");
	let start = Instant::now();

	for update in &updates {
		room.apply(None, &editor, update, limits).expect("applies");
	}

	let per_keystroke = start.elapsed() / updates.len() as u32;
	// The whole document is one backlog, as after a long store outage.
	let start = Instant::now();
	room.persist(rooms.store(), usize::MAX, false)
		.await
		.expect("persists");
	let backlog = start.elapsed();
	room.leave(membership.peer);

	// Measured on Apple silicon: 1.6 µs and 20 ms.
	assert_time(
		"applying one keystroke",
		per_keystroke,
		Duration::from_micros(20),
	);
	assert_time(
		"persisting a 42,000-update backlog",
		backlog,
		Duration::from_millis(150),
	);
}

/// A raw y-websocket peer.
type Socket =
	tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(fixture: &Fixture, document: &str, token: &str) -> Socket {
	let url = format!("{}/{document}?token={token}", fixture.sync_url());
	// Small client buffers, so the measurement is mostly the server's.
	let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
		.read_buffer_size(4096)
		.write_buffer_size(0);
	let (mut socket, _) = tokio_tungstenite::connect_async_with_config(&url, Some(config), false)
		.await
		.expect("connects");
	// The greeting is sync step 1; nobody has published presence.
	socket.next().await.expect("greets").expect("a frame");
	socket
}

/// A y-websocket update message inserting `index` into a shared text.
fn keystroke(doc: &Doc, text: &yrs::TextRef, index: u32) -> Vec<u8> {
	let update = {
		let mut txn = doc.transact_mut();
		text.insert(&mut txn, index, "x");
		txn.encode_update_v1()
	};
	canvist_cloud::sync::protocol::update(update)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "release-mode performance budget; run with `test:perf`"]
async fn one_update_fans_out_to_a_hundred_peers() {
	let fixture = Fixture::start().await;
	let token = fixture.token("fanout", Scope::Write);
	let mut sender = connect(&fixture, "fanout", &token).await;
	let mut peers = Vec::with_capacity(PEERS);

	for _ in 0..PEERS {
		peers.push(connect(&fixture, "fanout", &token).await);
	}

	let doc = Doc::with_client_id(77);
	let blocks = doc.get_or_insert_array("blocks");
	let text = {
		let mut txn = doc.transact_mut();
		let block = blocks.push_back(&mut txn, MapPrelim::default());
		block.insert(&mut txn, "type", "paragraph");
		block.insert(&mut txn, "text", TextPrelim::new(""))
	};
	let seed = {
		let txn = doc.transact();
		yrs::ReadTxn::encode_state_as_update_v1(&txn, &yrs::StateVector::default())
	};
	sender
		.send(Message::Binary(
			canvist_cloud::sync::protocol::update(seed).into(),
		))
		.await
		.expect("sends");

	for peer in &mut peers {
		peer.next()
			.await
			.expect("receives the seed")
			.expect("a frame");
	}

	let mut samples = Vec::with_capacity(SAMPLES);

	for index in 0..SAMPLES as u32 {
		let message = keystroke(&doc, &text, index);
		let start = Instant::now();
		sender
			.send(Message::Binary(message.into()))
			.await
			.expect("sends");

		for peer in &mut peers {
			peer.next().await.expect("receives").expect("a frame");
		}

		samples.push(start.elapsed());
	}

	samples.sort_unstable();
	assert_time(
		"one keystroke to the last of 100 peers",
		samples[SAMPLES / 2],
		Duration::from_millis(10),
	);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "release-mode performance budget; run with `test:perf`"]
async fn idle_connections_are_cheap() {
	const CONNECTIONS: usize = 1000;
	let fixture = Fixture::start().await;
	let tokens: Vec<String> = (0..CONNECTIONS / 10)
		.map(|index| fixture.token(&format!("room-{index}"), Scope::Write))
		.collect();
	// Load the rooms first, so the measurement is connections alone.
	let mut warm = Vec::new();

	for (index, token) in tokens.iter().enumerate() {
		warm.push(connect(&fixture, &format!("room-{index}"), token).await);
	}

	let before = live_bytes();
	let start = Instant::now();
	let mut sockets = Vec::with_capacity(CONNECTIONS);

	for index in 0..CONNECTIONS {
		let room = index % tokens.len();
		sockets.push(connect(&fixture, &format!("room-{room}"), &tokens[room]).await);
	}

	let elapsed = start.elapsed();
	tokio::time::sleep(Duration::from_millis(100)).await;
	// The test's client sockets live in this process too, so this is an
	// upper bound on the server's share.
	let per_connection = (live_bytes() - before).max(0) as usize / CONNECTIONS;
	eprintln!(
		"{CONNECTIONS} connections in {elapsed:?}, {} KiB per connection (client and server)",
		per_connection / 1024,
	);
	assert_bytes(
		"memory per idle connection, both ends",
		per_connection,
		64 * 1024,
	);
	assert_time(
		"opening one connection",
		elapsed / CONNECTIONS as u32,
		Duration::from_millis(2),
	);
	assert_eq!(
		fixture.server.app().rooms.stats().peers,
		CONNECTIONS + tokens.len()
	);
}

/// Median time of `samples` conversions of `input` from `from` to `to`
/// through `POST /v1/convert`.
async fn conversion_time(
	fixture: &Fixture,
	(from, to): (&str, &str),
	input: &serde_json::Value,
) -> Duration {
	const CONVERSIONS: usize = 5;
	let api = fixture.api();
	let mut samples = Vec::with_capacity(CONVERSIONS);

	for _ in 0..CONVERSIONS {
		let start = Instant::now();
		let response = api
			.post("/v1/convert")
			.json(&serde_json::json!({ "from": from, "to": to, "input": input }))
			.send()
			.await
			.expect("responds");
		assert_eq!(response.status(), 200, "{from} to {to}");
		response.bytes().await.expect("body");
		samples.push(start.elapsed());
	}

	samples.sort_unstable();
	samples[CONVERSIONS / 2]
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "release-mode performance budget; run with `test:perf`"]
async fn a_ten_thousand_paragraph_document_converts_quickly() {
	const PARAGRAPHS: usize = 10_000;
	let mut config = support::config();
	config.convert.max_input_bytes = 64 * 1024 * 1024;
	let fixture = Fixture::start_with(config, support::store().await).await;
	let markdown = serde_json::Value::String(vec![PARAGRAPH; PARAGRAPHS].join("\n\n"));
	let html: serde_json::Value = fixture
		.api()
		.post("/v1/convert")
		.json(&serde_json::json!({ "from": "markdown", "to": "html", "input": markdown }))
		.send()
		.await
		.expect("responds")
		.json()
		.await
		.expect("json");
	let json: serde_json::Value = fixture
		.api()
		.post("/v1/convert")
		.json(&serde_json::json!({ "from": "markdown", "to": "json", "input": markdown }))
		.send()
		.await
		.expect("responds")
		.json()
		.await
		.expect("json");

	let to_html = conversion_time(&fixture, ("markdown", "html"), &markdown).await;
	let from_html = conversion_time(&fixture, ("html", "markdown"), &html["output"]).await;
	let from_json = conversion_time(&fixture, ("json", "markdown"), &json["output"]).await;

	// Measured on Apple silicon, including HTTP and JSON: 14, 16, and
	// 52 ms.
	assert_time(
		"10,000 paragraphs from Markdown to HTML",
		to_html,
		Duration::from_millis(60),
	);
	assert_time(
		"10,000 paragraphs from HTML to Markdown",
		from_html,
		Duration::from_millis(60),
	);
	assert_time(
		"10,000 paragraphs from JSON to Markdown",
		from_json,
		Duration::from_millis(200),
	);
}
