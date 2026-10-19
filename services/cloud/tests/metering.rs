//! Monthly active documents: counting, free-plan caps that pause without
//! breaking clients, and Stripe meter events.

mod support;

use std::time::Duration;

use axum::http::HeaderMap;
use canvist_cloud::auth::Scope;
use canvist_cloud::config::Secret;
use canvist_cloud::config::StripeConfig;
use canvist_cloud::metering::Plan;
use canvist_cloud::metering::Terms;
use serde_json::Value as Json;
use serde_json::json;
use support::Fixture;
use support::deno::Client;
use support::deno::Transport;
use tokio::sync::mpsc;

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

/// A stand-in for Stripe that records meter event forms.
async fn fake_stripe() -> (String, mpsc::UnboundedReceiver<(HeaderMap, String)>) {
	let (sender, receiver) = mpsc::unbounded_channel();
	let app = axum::Router::new().route(
		"/v1/billing/meter_events",
		axum::routing::post(move |headers: HeaderMap, body: String| {
			let sender = sender.clone();
			async move {
				let _ = sender.send((headers, body));
				axum::Json(json!({ "object": "billing.meter_event" }))
			}
		}),
	);
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
		.await
		.expect("binds");
	let url = format!("http://{}", listener.local_addr().expect("addr"));
	tokio::spawn(async move { axum::serve(listener, app).await });
	(url, receiver)
}

#[tokio::test(flavor = "multi_thread")]
async fn new_paid_documents_send_one_stripe_meter_event_each() {
	let (stripe, mut events) = fake_stripe().await;
	let mut config = support::config();
	config.stripe = Some(StripeConfig {
		secret_key: Secret::new("sk_test_canvist"),
		meter_event: "canvist_active_document".to_owned(),
		api_base: stripe,
	});
	let fixture = Fixture::start_with(config, support::store().await).await;
	let terms = Terms {
		stripe_customer: Some("cus_test123".to_owned()),
		..Terms::of(Plan::Builder)
	};
	fixture
		.store
		.set_terms(&fixture.project, &terms)
		.await
		.expect("sets terms");
	let api = fixture.api();

	for name in ["a", "b", "a"] {
		api.post(&format!("/v1/documents/{name}"))
			.body(paragraph(1))
			.send()
			.await
			.expect("posts");
	}

	let mut identifiers = Vec::new();

	for _ in 0..2 {
		let (headers, form) = tokio::time::timeout(Duration::from_secs(5), events.recv())
			.await
			.expect("an event arrives")
			.expect("open");
		assert_eq!(headers["authorization"], "Bearer sk_test_canvist");
		let fields: Vec<(String, String)> = form
			.split('&')
			.filter_map(|pair| pair.split_once('='))
			.map(|(key, value)| (decode(key), decode(value)))
			.collect();
		let field = |name: &str| {
			fields
				.iter()
				.find(|(key, _)| key == name)
				.map(|(_, value)| value.clone())
		};
		assert_eq!(
			field("event_name").as_deref(),
			Some("canvist_active_document")
		);
		assert_eq!(
			field("payload[stripe_customer_id]").as_deref(),
			Some("cus_test123")
		);
		assert_eq!(field("payload[value]").as_deref(), Some("1"));
		identifiers.push(field("identifier").expect("an identifier"));
	}

	assert_ne!(identifiers[0], identifiers[1]);
	tokio::time::sleep(Duration::from_millis(300)).await;
	assert!(events.try_recv().is_err(), "a document counts once a month");
}

fn decode(text: &str) -> String {
	let bytes = text.as_bytes();
	let mut out = Vec::new();
	let mut index = 0;

	while index < bytes.len() {
		match bytes[index] {
			b'%' => {
				let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).expect("ascii");
				out.push(u8::from_str_radix(hex, 16).expect("hex"));
				index += 3;
			}
			b'+' => {
				out.push(b' ');
				index += 1;
			}
			byte => {
				out.push(byte);
				index += 1;
			}
		}
	}

	String::from_utf8(out).expect("utf-8")
}

/// `body` signed as Stripe signs webhooks.
fn stripe_signature(secret: &str, body: &str) -> String {
	use hmac::Mac;

	let timestamp = canvist_cloud::auth::unix_now();
	let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).expect("any key");
	mac.update(format!("{timestamp}.{body}").as_bytes());
	let digest = mac.finalize().into_bytes();
	let hex = digest.iter().fold(String::new(), |mut out, byte| {
		use std::fmt::Write;
		let _ = write!(out, "{byte:02x}");
		out
	});
	format!("t={timestamp},v1={hex}")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stripe_subscription_lifts_the_cap() {
	const SECRET: &str = "whsec_canvist_test";
	let mut config = support::config();
	config.sync.pause_recheck = Duration::from_millis(200);
	config.stripe_webhook_secret = Some(Secret::new(SECRET));
	let fixture = Fixture::start_with(config, support::store().await).await;
	let capped_terms = Terms {
		included_documents: Some(0),
		..Terms::of(Plan::Hobby)
	};
	fixture
		.store
		.set_terms(&fixture.project, &capped_terms)
		.await
		.expect("sets terms");
	let token = fixture.token("*", Scope::Write);
	let mut client = Client::spawn();
	let attempt = client.connect_within(
		Transport::YWebsocket,
		(&fixture.sync_url(), "doc", &token),
		1000,
	);
	assert!(!attempt.synced);

	let body = json!({
		"type": "customer.subscription.created",
		"data": { "object": {
			"customer": "cus_test",
			"status": "active",
			"metadata": { "canvist_project": fixture.project.as_str() },
			"items": { "data": [{ "price": { "lookup_key": "canvist_builder_monthly" } }] },
		} },
	})
	.to_string();
	let http = reqwest::Client::new();
	let forged = http
		.post(fixture.http("/v1/stripe/webhook"))
		.header("stripe-signature", stripe_signature("whsec_wrong", &body))
		.body(body.clone())
		.send()
		.await
		.expect("responds");
	assert_eq!(forged.status(), 400);

	let genuine = http
		.post(fixture.http("/v1/stripe/webhook"))
		.header("stripe-signature", stripe_signature(SECRET, &body))
		.body(body)
		.send()
		.await
		.expect("responds");
	assert_eq!(genuine.status(), 200);
	assert!(
		client.wait_synced(),
		"the paused session syncs once Stripe reports the upgrade"
	);
	assert_eq!(usage(&fixture).await["plan"], "builder");
}
