//! The REST API, webhooks, eviction, and graceful shutdown.

mod support;

use std::time::Duration;

use axum::http::HeaderMap;
use canvist_cloud::auth::Scope;
use futures_util::StreamExt;
use serde_json::Value as Json;
use support::Fixture;
use support::deno::Client;
use support::deno::Transport;
use tokio::sync::mpsc;
use yrs::Array;
use yrs::Doc;
use yrs::GetString;
use yrs::Map;
use yrs::MapPrelim;
use yrs::Text;
use yrs::TextPrelim;
use yrs::Transact;
use yrs::Update;
use yrs::updates::decoder::Decode;

/// A v1 update appending a paragraph holding `text`, from client `client`.
fn paragraph(client: u64, text: &str) -> Vec<u8> {
	let doc = Doc::with_client_id(client);
	let blocks = doc.get_or_insert_array("blocks");
	let mut txn = doc.transact_mut();
	let block = blocks.push_back(&mut txn, MapPrelim::default());
	block.insert(&mut txn, "id", format!("{client}.1"));
	block.insert(&mut txn, "type", "paragraph");
	block.insert(&mut txn, "attrs", MapPrelim::default());
	block.insert(&mut txn, "text", TextPrelim::new(text));
	txn.encode_update_v1()
}

/// The paragraph texts in a v1 state.
fn texts(state: &[u8]) -> Vec<String> {
	let doc = Doc::new();
	let blocks = doc.get_or_insert_array("blocks");
	doc.transact_mut()
		.apply_update(Update::decode_v1(state).expect("decodes"))
		.expect("applies");
	let txn = doc.transact();
	blocks
		.iter(&txn)
		.filter_map(|block| {
			let block = block.cast::<yrs::MapRef>().ok()?;
			let text = block.get(&txn, "text")?.cast::<yrs::TextRef>().ok()?;
			Some(text.get_string(&txn))
		})
		.collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn servers_write_and_read_documents_over_rest() {
	let fixture = Fixture::start().await;
	let api = fixture.api();
	let mut client = Client::spawn();
	let token = fixture.token("report", Scope::Write);
	assert!(
		client
			.connect(Transport::YWebsocket, &fixture.sync_url(), "report", &token)
			.synced
	);

	let response = api
		.post("/v1/documents/report")
		.body(paragraph(900, "from the server"))
		.send()
		.await
		.expect("posts");
	assert_eq!(response.status(), 200);
	assert_eq!(
		client.wait_texts(&["from the server"]),
		["from the server"],
		"connected peers receive it at once"
	);

	client.append("from a client");
	client.wait_texts(&["from the server", "from a client"]);
	tokio::time::sleep(Duration::from_millis(100)).await;
	let state = api
		.get("/v1/documents/report")
		.send()
		.await
		.expect("exports")
		.bytes()
		.await
		.expect("body");
	assert_eq!(texts(&state), ["from the server", "from a client"]);

	assert_eq!(
		api.delete("/v1/documents/report")
			.send()
			.await
			.expect("deletes")
			.status(),
		204
	);
	assert_eq!(
		api.get("/v1/documents/report")
			.send()
			.await
			.expect("gets")
			.status(),
		404
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn updates_outside_the_canvist_schema_are_refused() {
	let fixture = Fixture::start().await;
	let api = fixture.api();
	let foreign = {
		let doc = Doc::with_client_id(5);
		let text = doc.get_or_insert_text("prosemirror");
		let mut txn = doc.transact_mut();
		text.insert(&mut txn, 0, "not canvist");
		txn.encode_update_v1()
	};
	let bare = {
		let doc = Doc::with_client_id(6);
		let blocks = doc.get_or_insert_array("blocks");
		let mut txn = doc.transact_mut();
		blocks.push_back(&mut txn, "a string is not a block");
		txn.encode_update_v1()
	};

	for update in [foreign, bare, vec![1, 2, 3]] {
		let response = api
			.post("/v1/documents/doc")
			.body(update)
			.send()
			.await
			.expect("posts");
		assert_eq!(response.status(), 422);
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn client_tokens_cannot_use_the_rest_api() {
	let fixture = Fixture::start().await;
	let client = reqwest::Client::new();
	let token = fixture.token("*", Scope::Write);

	let response = client
		.get(fixture.http("/v1/documents"))
		.bearer_auth(&token)
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), 403);
	let response = client
		.get(fixture.http("/v1/documents"))
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn api_keys_mint_scoped_client_tokens() {
	let fixture = Fixture::start().await;
	let api = fixture.api();
	let minted: Json = api
		.post("/v1/tokens")
		.json(&serde_json::json!({ "subject": "ifiok", "document": "team/*", "scope": "read" }))
		.send()
		.await
		.expect("mints")
		.json()
		.await
		.expect("json");
	let token = minted["token"].as_str().expect("a token");
	let mut client = Client::spawn();
	assert!(
		client
			.connect(
				Transport::YWebsocket,
				&fixture.sync_url(),
				"team/notes",
				token
			)
			.synced
	);

	let too_long = api
		.post("/v1/tokens")
		.json(&serde_json::json!({ "subject": "x", "document": "d", "ttl_seconds": 999_999 }))
		.send()
		.await
		.expect("responds");
	assert_eq!(too_long.status(), 400);
}

/// A local endpoint that records webhook deliveries.
async fn webhook_receiver() -> (String, mpsc::UnboundedReceiver<(HeaderMap, String)>) {
	let (sender, receiver) = mpsc::unbounded_channel();
	let app = axum::Router::new().route(
		"/hook",
		axum::routing::post(move |headers: HeaderMap, body: String| {
			let sender = sender.clone();
			async move {
				let _ = sender.send((headers, body));
				"ok"
			}
		}),
	);
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
		.await
		.expect("binds");
	let url = format!("http://{}/hook", listener.local_addr().expect("addr"));
	tokio::spawn(async move { axum::serve(listener, app).await });
	(url, receiver)
}

#[tokio::test(flavor = "multi_thread")]
async fn edits_fire_one_debounced_signed_webhook() {
	let fixture = Fixture::start().await;
	let api = fixture.api();
	let (url, mut deliveries) = webhook_receiver().await;
	let configured: Json = api
		.put("/v1/webhook")
		.json(&serde_json::json!({ "url": url }))
		.send()
		.await
		.expect("sets")
		.json()
		.await
		.expect("json");
	let secret = configured["secret"].as_str().expect("a secret").to_owned();

	for (client, text) in [(1, "one"), (2, "two"), (3, "three")] {
		api.post("/v1/documents/watched")
			.body(paragraph(client, text))
			.send()
			.await
			.expect("posts");
	}

	let (headers, body) = tokio::time::timeout(Duration::from_secs(5), deliveries.recv())
		.await
		.expect("a delivery arrives")
		.expect("open");
	let payload: Json = serde_json::from_str(&body).expect("json");
	assert_eq!(payload["type"], "document.updated");
	assert_eq!(payload["document"], "watched");
	assert_eq!(payload["editors"], serde_json::json!(["server"]));

	let signature = headers["canvist-signature"].to_str().expect("ascii");
	let timestamp: u64 = signature
		.split(',')
		.find_map(|part| part.strip_prefix("t="))
		.and_then(|t| t.parse().ok())
		.expect("a timestamp");
	assert_eq!(
		signature,
		canvist_cloud::sync::webhooks::sign(&secret, timestamp, &body)
	);

	tokio::time::sleep(Duration::from_millis(500)).await;
	assert!(
		deliveries.try_recv().is_err(),
		"three quick edits make one delivery"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn private_webhook_targets_are_refused_in_production_config() {
	let mut config = support::config();
	config.sync.webhook_allow_private = false;
	let fixture = Fixture::start_with(config, support::store().await).await;
	let response = fixture
		.api()
		.put("/v1/webhook")
		.json(&serde_json::json!({ "url": "http://127.0.0.1:9/hook" }))
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn idle_documents_are_compacted_and_evicted() {
	let mut config = support::config();
	config.sync.idle_eviction = Duration::from_millis(100);
	let fixture = Fixture::start_with(config, support::store().await).await;
	let api = fixture.api();
	api.post("/v1/documents/idle")
		.body(paragraph(1, "remember me"))
		.send()
		.await
		.expect("posts");
	assert_eq!(fixture.server.app().rooms.stats().rooms, 1);

	let app = fixture.server.app().clone();
	assert!(
		support::eventually(|| app.rooms.stats().rooms == 0).await,
		"the idle room is evicted"
	);

	let listed: Json = api
		.get("/v1/documents")
		.send()
		.await
		.expect("lists")
		.json()
		.await
		.expect("json");
	assert_eq!(
		listed["documents"][0]["pending_updates"], 0,
		"eviction compacts"
	);
	let state = api
		.get("/v1/documents/idle")
		.send()
		.await
		.expect("exports")
		.bytes()
		.await
		.expect("body");
	assert_eq!(
		texts(&state),
		["remember me"],
		"the evicted document reloads from its snapshot"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_closes_sockets_with_service_restart() {
	let fixture = Fixture::start().await;
	let url = format!(
		"{}/doc?token={}",
		fixture.sync_url(),
		fixture.token("doc", Scope::Write)
	);
	let (mut socket, _) = tokio_tungstenite::connect_async(&url)
		.await
		.expect("connects");
	// The greeting: sync step 1.
	assert!(socket.next().await.is_some());

	let shutdown = tokio::spawn(fixture.server.shutdown());
	let close = loop {
		match socket.next().await {
			Some(Ok(tokio_tungstenite::tungstenite::Message::Close(frame))) => break frame,
			Some(Ok(_)) => {}
			other => panic!("expected a close frame, got {other:?}"),
		}
	};
	assert_eq!(close.map(|frame| u16::from(frame.code)), Some(1012));
	drop(socket);
	assert_eq!(shutdown.await.expect("joins").persist_failures, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn health_and_readiness() {
	let fixture = Fixture::start().await;
	let client = reqwest::Client::new();
	let health = client
		.get(fixture.http("/healthz"))
		.send()
		.await
		.expect("responds");
	assert_eq!(health.status(), 200);
	let ready = client
		.get(fixture.http("/readyz"))
		.send()
		.await
		.expect("responds");
	assert_eq!(ready.status(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn admins_create_projects_with_keys_per_environment() {
	let fixture = Fixture::start().await;
	let client = reqwest::Client::new();
	let created: Json = client
		.post(fixture.http("/v1/admin/projects"))
		.bearer_auth(support::ADMIN)
		.json(&serde_json::json!({ "name": "acme", "environments": ["production", "staging"] }))
		.send()
		.await
		.expect("creates")
		.json()
		.await
		.expect("json");
	let staging = created["keys"]["staging"].as_str().expect("a staging key");
	let mut client_js = Client::spawn();
	assert!(
		client_js
			.connect(Transport::YWebsocket, &fixture.sync_url(), "doc", staging)
			.synced
	);

	let refused = client
		.post(fixture.http("/v1/admin/projects"))
		.bearer_auth("wrong")
		.json(&serde_json::json!({ "name": "x" }))
		.send()
		.await
		.expect("responds");
	assert_eq!(refused.status(), 401);
}
