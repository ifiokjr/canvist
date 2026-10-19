//! Monthly active documents: counting, free-plan caps that pause without
//! breaking clients. Stripe billing is tested in `billing.rs`.

mod support;

use std::time::Duration;

use canvist_cloud::auth::Scope;
use canvist_cloud::metering::Plan;
use canvist_cloud::metering::Terms;
use serde_json::Value as Json;
use serde_json::json;
use support::Fixture;
use support::deno::Client;
use support::deno::Transport;

/// A free plan capped at `cap` documents, rechecking paused sessions fast.
async fn capped(cap: u64) -> Fixture {
	let mut config = support::config();
	config.sync.pause_recheck = Duration::from_millis(200);
	let fixture = Fixture::start_with(config, support::store().await).await;
	let terms = Terms {
		included_documents: Some(cap),
		..Terms::of(Plan::Hobby)
	};
	fixture
		.store
		.set_terms(&fixture.project, &terms)
		.await
		.expect("sets terms");
	fixture
}

async fn upgrade(fixture: &Fixture, plan: &str) {
	let response = reqwest::Client::new()
		.put(fixture.http(&format!("/v1/admin/projects/{}/terms", fixture.project)))
		.bearer_auth(support::ADMIN)
		.json(&json!({ "plan": plan }))
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), 200);
}

async fn usage(fixture: &Fixture) -> Json {
	fixture
		.api()
		.get("/v1/usage")
		.send()
		.await
		.expect("responds")
		.json()
		.await
		.expect("json")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_free_plan_pauses_new_documents_until_an_upgrade() {
	let fixture = capped(1).await;
	let url = fixture.sync_url();
	let token = fixture.token("*", Scope::Write);
	let mut first = Client::spawn();
	let mut second = Client::spawn();
	let mut paused = Client::spawn();

	assert!(
		first
			.connect(Transport::YWebsocket, &url, "first", &token)
			.synced
	);
	let attempt = paused.connect_within(Transport::YWebsocket, (&url, "second", &token), 1000);
	assert!(!attempt.synced, "the free plan's only document is taken");

	// The paused client keeps editing locally, and the existing document
	// keeps syncing for everyone.
	paused.append("written while paused");
	first.append("still syncing");
	assert!(
		second
			.connect(Transport::YWebsocket, &url, "first", &token)
			.synced
	);
	assert_eq!(second.wait_texts(&["still syncing"]), ["still syncing"]);
	assert_eq!(usage(&fixture).await["standing"], "capped");

	upgrade(&fixture, "builder").await;
	assert!(
		paused.wait_synced(),
		"the paused session syncs after the upgrade"
	);

	let mut reader = Client::spawn();
	assert!(
		reader
			.connect(Transport::YWebsocket, &url, "second", &token)
			.synced
	);
	assert_eq!(
		reader.wait_texts(&["written while paused"]),
		["written while paused"],
		"what was written while paused reaches the relay"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn hocuspocus_sessions_pause_without_failing_authentication() {
	let fixture = capped(0).await;
	let token = fixture.token("*", Scope::Write);
	let mut client = Client::spawn();
	let attempt = client.connect_within(
		Transport::Hocuspocus,
		(&fixture.hocuspocus_url(), "doc", &token),
		1000,
	);
	assert_eq!(
		attempt.denied, None,
		"pausing is not an authentication failure"
	);
	assert!(!attempt.synced);
	client.append("local");

	upgrade(&fixture, "scale").await;
	assert!(client.wait_synced());
	let mut reader = Client::spawn();
	assert!(
		reader
			.connect(Transport::YWebsocket, &fixture.sync_url(), "doc", &token)
			.synced
	);
	assert_eq!(reader.wait_texts(&["local"]), ["local"]);
}

/// A v1 update appending one paragraph.
fn paragraph(client: u64) -> Vec<u8> {
	use yrs::Array;
	use yrs::Map;
	use yrs::MapPrelim;
	use yrs::Transact;

	let doc = yrs::Doc::with_client_id(client);
	let blocks = doc.get_or_insert_array("blocks");
	let mut txn = doc.transact_mut();
	let block = blocks.push_back(&mut txn, MapPrelim::default());
	block.insert(&mut txn, "type", "paragraph");
	txn.encode_update_v1()
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_writes_never_exceed_the_cap() {
	const CAP: usize = 5;
	const WRITERS: usize = 20;
	let fixture = capped(CAP as u64).await;
	let api = fixture.api();
	let writes = (0..WRITERS).map(|index| {
		api.post(&format!("/v1/documents/doc-{index}"))
			.body(paragraph(index as u64 + 1))
			.send()
	});
	let statuses: Vec<u16> = futures_util::future::join_all(writes)
		.await
		.into_iter()
		.map(|response| response.expect("responds").status().as_u16())
		.collect();

	assert_eq!(
		statuses.iter().filter(|status| **status == 200).count(),
		CAP
	);
	assert_eq!(
		statuses.iter().filter(|status| **status == 429).count(),
		WRITERS - CAP
	);

	let admitted = statuses
		.iter()
		.position(|status| *status == 200)
		.expect("one was admitted");
	let again = api
		.post(&format!("/v1/documents/doc-{admitted}"))
		.body(paragraph(98))
		.send()
		.await
		.expect("responds");
	assert_eq!(
		again.status(),
		200,
		"admitted documents keep accepting writes"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn usage_counts_each_document_once_a_month() {
	let fixture = Fixture::start().await;
	let api = fixture.api();
	let mut client = Client::spawn();
	let token = fixture.token("*", Scope::Write);

	for _ in 0..3 {
		api.post("/v1/documents/repeated")
			.body(paragraph(1))
			.send()
			.await
			.expect("posts");
	}

	assert!(
		client
			.connect(
				Transport::YWebsocket,
				&fixture.sync_url(),
				"repeated",
				&token
			)
			.synced
	);
	client.disconnect();
	api.post("/v1/documents/other")
		.body(paragraph(2))
		.send()
		.await
		.expect("posts");

	let usage = usage(&fixture).await;
	assert_eq!(usage["plan"], "hobby");
	assert_eq!(usage["included_documents"], 1000);
	assert_eq!(usage["active_documents"], 2);
	assert_eq!(usage["standing"], "ok");
	assert_eq!(usage["environments"][0]["environment"], "production");
	assert_eq!(usage["environments"][0]["active_documents"], 2);

	let past: Json = api
		.get("/v1/usage?month=2001-01")
		.send()
		.await
		.expect("responds")
		.json()
		.await
		.expect("json");
	assert_eq!(past["active_documents"], 0);
	let invalid = api
		.get("/v1/usage?month=January")
		.send()
		.await
		.expect("responds");
	assert_eq!(invalid.status(), 400);
}
