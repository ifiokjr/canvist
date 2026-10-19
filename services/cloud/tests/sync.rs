//! Stock JavaScript Yjs clients converge through the relay, documents
//! survive restarts, and bad credentials are refused.

mod support;

use canvist_cloud::auth::Scope;
use support::Fixture;
use support::deno::Client;
use support::deno::Transport;

#[tokio::test(flavor = "multi_thread")]
async fn two_y_websocket_clients_converge_with_presence() {
	let fixture = Fixture::start().await;
	let token = fixture.token("notes", Scope::Write);
	let mut alice = Client::spawn();
	let mut bob = Client::spawn();

	assert!(
		alice
			.connect(Transport::YWebsocket, &fixture.sync_url(), "notes", &token)
			.synced
	);
	assert!(
		bob.connect(Transport::YWebsocket, &fixture.sync_url(), "notes", &token)
			.synced
	);

	alice.append("Hello");
	assert_eq!(bob.wait_texts(&["Hello"]), ["Hello"]);
	bob.append("world");
	alice.presence("Alice");
	bob.presence("Bob");

	let expected = ["Hello", "world"];
	assert_eq!(alice.wait(&expected, 1).texts, expected);
	assert_eq!(
		bob.wait(&expected, 1).peers,
		1,
		"each sees the other's presence"
	);

	alice.disconnect();
	assert_eq!(
		bob.wait(&expected, 0).peers,
		0,
		"presence is withdrawn on disconnect"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn hocuspocus_and_y_websocket_clients_share_a_document() {
	let fixture = Fixture::start().await;
	let token = fixture.token("team/*", Scope::Write);
	let mut hocuspocus = Client::spawn();
	let mut websocket = Client::spawn();

	assert!(
		hocuspocus
			.connect(
				Transport::Hocuspocus,
				&fixture.hocuspocus_url(),
				"team/plan",
				&token
			)
			.synced
	);
	assert!(
		websocket
			.connect(
				Transport::YWebsocket,
				&fixture.sync_url(),
				"team/plan",
				&token
			)
			.synced
	);

	hocuspocus.append("from hocuspocus");
	assert_eq!(
		websocket.wait_texts(&["from hocuspocus"]),
		["from hocuspocus"]
	);
	websocket.append("from y-websocket");
	let expected = ["from hocuspocus", "from y-websocket"];
	assert_eq!(hocuspocus.wait_texts(&expected), expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn documents_survive_a_restart() {
	let fixture = Fixture::start().await;
	let token = fixture.token("durable", Scope::Write);
	let mut writer = Client::spawn();
	assert!(
		writer
			.connect(
				Transport::YWebsocket,
				&fixture.sync_url(),
				"durable",
				&token
			)
			.synced
	);
	writer.append("kept");
	// Let the edit reach the relay before the restart.
	writer.wait_texts(&["kept"]);
	tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	drop(writer);

	let fixture = fixture.restart(support::config()).await;
	let mut reader = Client::spawn();
	assert!(
		reader
			.connect(
				Transport::YWebsocket,
				&fixture.sync_url(),
				"durable",
				&token
			)
			.synced
	);
	assert_eq!(reader.wait_texts(&["kept"]), ["kept"]);

	let documents: serde_json::Value = fixture
		.api()
		.get("/v1/documents")
		.send()
		.await
		.expect("lists")
		.json()
		.await
		.expect("json");
	assert_eq!(documents["documents"][0]["name"], "durable");
	assert_eq!(
		documents["documents"][0]["pending_updates"], 0,
		"shutdown compacts every document into a snapshot"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn read_only_clients_see_edits_but_cannot_make_them() {
	let fixture = Fixture::start().await;
	let mut editor = Client::spawn();
	let mut viewer = Client::spawn();
	let url = fixture.sync_url();

	assert!(
		editor
			.connect(
				Transport::YWebsocket,
				&url,
				"doc",
				&fixture.token("doc", Scope::Write)
			)
			.synced
	);
	assert!(
		viewer
			.connect(
				Transport::YWebsocket,
				&url,
				"doc",
				&fixture.token("doc", Scope::Read)
			)
			.synced
	);

	viewer.append("vandalism");
	editor.append("real");
	assert_eq!(
		viewer.wait_texts(&["vandalism", "real"]).len(),
		2,
		"the viewer keeps its local edit"
	);
	tokio::time::sleep(std::time::Duration::from_millis(200)).await;
	assert_eq!(
		editor.wait_texts(&["real"]),
		["real"],
		"the relay refused the viewer's edit"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_credentials_are_refused_before_the_upgrade() {
	let fixture = Fixture::start().await;
	let base = fixture.sync_url();
	let expired = fixture.token_until("doc", Scope::Write, 1);
	let other_document = fixture.token("other", Scope::Write);
	let forged = forge(&fixture);

	for (url, status) in [
		(format!("{base}/doc"), 401),
		(format!("{base}/doc?token=garbage"), 401),
		(format!("{base}/doc?token={expired}"), 401),
		(format!("{base}/doc?token={forged}"), 401),
		(format!("{base}/doc?token={other_document}"), 403),
		(format!("{base}/doc?token=ck_unknownkey"), 401),
		(format!("{base}/bad%20name?token={other_document}"), 400),
	] {
		match tokio_tungstenite::connect_async(&url).await {
			Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
				assert_eq!(response.status().as_u16(), status, "{url}");
			}
			other => panic!("{url} should be refused, got {other:?}"),
		}
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn hocuspocus_reports_permission_denied() {
	let fixture = Fixture::start().await;
	let mut client = Client::spawn();
	let connected = client.connect(
		Transport::Hocuspocus,
		&fixture.hocuspocus_url(),
		"secret",
		&fixture.token("public", Scope::Write),
	);
	assert!(!connected.synced);
	assert!(
		connected
			.denied
			.is_some_and(|reason| reason.contains("cannot open"))
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn api_keys_open_any_document_in_their_environment() {
	let fixture = Fixture::start().await;
	let mut client = Client::spawn();
	let connected = client.connect(
		Transport::YWebsocket,
		&fixture.sync_url(),
		"anything",
		&fixture.api_key,
	);
	assert!(connected.synced);
}

/// A well-formed token for the fixture's project, signed with another key.
fn forge(fixture: &Fixture) -> String {
	let genuine = fixture.token("doc", Scope::Write);
	let claims = canvist_cloud::auth::TokenSigner::new(support::SECRET)
		.verify(&genuine, canvist_cloud::auth::unix_now())
		.expect("genuine");
	canvist_cloud::auth::TokenSigner::new("an attacker's key that is long enough").sign(&claims)
}
