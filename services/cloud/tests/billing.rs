//! Stripe billing: usage reported through the outbox, Checkout for monthly
//! and annual plans, plan switches, the Customer Portal, and subscription
//! webhooks. Stripe is the fake in `support/stripe.rs`.

mod support;

use std::time::Duration;

use canvist_cloud::Config;
use canvist_cloud::auth::Scope;
use canvist_cloud::config::Secret;
use canvist_cloud::metering::Month;
use canvist_cloud::metering::Plan;
use canvist_cloud::metering::Terms;
use serde_json::Value as Json;
use serde_json::json;
use support::Fixture;
use support::deno::Client;
use support::deno::Transport;
use support::stripe::FakeStripe;

const WEBHOOK_SECRET: &str = "whsec_canvist_test";

fn billing_config(stripe: &FakeStripe) -> Config {
	let mut config = support::config();
	config.stripe = Some(stripe.config());
	config.stripe_webhook_secret = Some(Secret::new(WEBHOOK_SECRET));
	config.sync.pause_recheck = Duration::from_millis(200);
	config
}

/// A server with Stripe over its own database: every server with Stripe
/// reports its whole database's usage, so billing tests never share one.
async fn start(stripe: &FakeStripe) -> Fixture {
	start_over(stripe, support::isolated_database().await.as_deref()).await
}

async fn start_over(stripe: &FakeStripe, database: Option<&str>) -> Fixture {
	Fixture::start_with(billing_config(stripe), support::store_at(database).await).await
}

/// Put the fixture's project on Builder, billed to `cus_test`.
async fn on_builder(fixture: &Fixture) {
	let terms = Terms {
		stripe_customer: Some("cus_test".to_owned()),
		..Terms::of(Plan::Builder)
	};
	fixture
		.store
		.set_terms(&fixture.project, &terms)
		.await
		.expect("sets terms");
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

async fn write(fixture: &Fixture, name: &str) {
	let response = fixture
		.api()
		.post(&format!("/v1/documents/{name}"))
		.body(paragraph(1))
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), 200);
}

async fn convert(fixture: &Fixture) {
	let response = fixture
		.api()
		.post("/v1/convert")
		.json(&json!({ "from": "text", "to": "html", "input": "x" }))
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

/// Deliver a signed event of `kind` about `object`, returning the response.
async fn deliver(fixture: &Fixture, kind: &str, object: Json) -> reqwest::Response {
	let body = json!({ "type": kind, "data": { "object": object } }).to_string();
	reqwest::Client::new()
		.post(fixture.http("/v1/stripe/webhook"))
		.header("stripe-signature", stripe_signature(WEBHOOK_SECRET, &body))
		.body(body)
		.send()
		.await
		.expect("responds")
}

async fn post(fixture: &Fixture, path: &str, body: Json) -> reqwest::Response {
	fixture
		.api()
		.post(path)
		.json(&body)
		.send()
		.await
		.expect("responds")
}

#[tokio::test(flavor = "multi_thread")]
async fn new_paid_documents_send_one_meter_event_each() {
	let mut stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	write(&fixture, "before").await;
	on_builder(&fixture).await;

	for name in ["a", "b", "a"] {
		write(&fixture, name).await;
	}

	let mut identifiers = Vec::new();

	for _ in 0..2 {
		let event = stripe.next_meter_event().await;
		assert_eq!(event.headers["authorization"], "Bearer sk_test_canvist");
		assert_eq!(event.headers["stripe-version"], "2026-08-26.dahlia");
		assert!(event.headers.contains_key("idempotency-key"));
		assert_eq!(event.field("event_name"), Some("canvist_active_document"));
		assert_eq!(event.field("payload[stripe_customer_id]"), Some("cus_test"));
		assert_eq!(event.field("payload[value]"), Some("1"));
		identifiers.push(event.field("identifier").expect("an identifier").to_owned());
	}

	assert_ne!(identifiers[0], identifiers[1]);
	assert!(
		stripe
			.no_meter_event_within(Duration::from_millis(300))
			.await,
		"a document counts once a month, and Hobby documents are never billed"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn usage_survives_stripe_outages_and_restarts() {
	let mut stripe = FakeStripe::start().await;
	let database = support::isolated_database().await;
	let fixture = start_over(&stripe, database.as_deref()).await;
	on_builder(&fixture).await;
	stripe.fail_meter_events(usize::MAX);
	write(&fixture, "during-the-outage").await;
	tokio::time::sleep(Duration::from_millis(300)).await;
	assert!(
		stripe.posts("/v1/billing/meter_events").len() > 1,
		"the reporter keeps retrying"
	);

	// A deploy: a new process over the same database.
	let store = match &database {
		Some(_) => support::store_at(database.as_deref()).await,
		None => fixture.store.clone(),
	};
	// Kept alive until the end, so its reporter is the one that delivers.
	let _restarted = fixture.restart_with(billing_config(&stripe), store).await;
	stripe.fail_meter_events(0);
	let event = stripe.next_meter_event().await;
	assert_eq!(event.field("payload[value]"), Some("1"));
	assert!(
		stripe
			.no_meter_event_within(Duration::from_millis(300))
			.await,
		"once accepted, it is not sent again"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn conversions_are_reported_as_changes_in_the_count() {
	let mut stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	convert(&fixture).await;
	on_builder(&fixture).await;

	// While Stripe is down the conversions pile up; afterwards they arrive
	// in as few events as the reservations allow, adding up exactly.
	stripe.fail_meter_events(usize::MAX);
	convert(&fixture).await;
	convert(&fixture).await;
	tokio::time::sleep(Duration::from_millis(200)).await;
	stripe.fail_meter_events(0);

	let mut total = 0;
	let mut identifiers = Vec::new();

	while total < 2 {
		let event = stripe.next_meter_event().await;
		assert_eq!(event.field("event_name"), Some("canvist_conversion"));
		total += event
			.field("payload[value]")
			.and_then(|value| value.parse::<u64>().ok())
			.expect("a count");
		identifiers.push(event.field("identifier").expect("an identifier").to_owned());
	}

	assert_eq!(total, 2, "the Hobby conversion is never billed");
	convert(&fixture).await;
	let event = stripe.next_meter_event().await;
	assert_eq!(event.field("payload[value]"), Some("1"));
	assert!(!identifiers.contains(&event.field("identifier").expect("an identifier").to_owned()));
	assert!(
		stripe
			.no_meter_event_within(Duration::from_millis(300))
			.await
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn checkout_starts_a_subscription_anchored_to_the_month() {
	let stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	let response = post(
		&fixture,
		"/v1/billing/checkout",
		json!({
			"plan": "builder",
			"success_url": "https://app.example/billing/done",
			"cancel_url": "https://app.example/billing",
		}),
	)
	.await;
	assert_eq!(response.status(), 200);
	let body: Json = response.json().await.expect("json");
	assert_eq!(body["url"], "https://checkout.stripe.test/cs_test_1");

	let [session] = stripe
		.posts("/v1/checkout/sessions")
		.try_into()
		.expect("one session");
	let project = fixture.project.as_str();
	let anchor = Month::current().next().start_unix_seconds().to_string();
	assert_eq!(session.field("mode"), Some("subscription"));
	assert_eq!(
		session.field("line_items[0][price]"),
		Some("price_canvist_builder_monthly")
	);
	assert_eq!(session.field("line_items[0][quantity]"), Some("1"));
	assert_eq!(
		session.field("line_items[1][price]"),
		Some("price_canvist_builder_mad_overage")
	);
	assert_eq!(
		session.field("line_items[1][quantity]"),
		None,
		"metered prices take no quantity"
	);
	assert_eq!(
		session.field("line_items[2][price]"),
		Some("price_canvist_builder_conversion_overage")
	);
	assert_eq!(
		session.field("subscription_data[metadata][canvist_project]"),
		Some(project)
	);
	assert_eq!(session.field("client_reference_id"), Some(project));
	assert_eq!(
		session.field("subscription_data[billing_cycle_anchor]"),
		Some(anchor.as_str())
	);
	assert_eq!(session.field("tax_id_collection[enabled]"), Some("true"));
	assert_eq!(
		session.field("automatic_tax[enabled]"),
		None,
		"tax stays off until switched on"
	);
	assert_eq!(
		session.field("customer"),
		None,
		"Checkout creates the first customer"
	);
	assert_eq!(session.field("payment_method_types[0]"), None);

	for plan in ["hobby", "enterprise"] {
		let refused = post(
			&fixture,
			"/v1/billing/checkout",
			json!({ "plan": plan, "success_url": "https://a.example", "cancel_url": "https://a.example" }),
		)
		.await;
		assert_eq!(refused.status(), 400, "{plan} is not sold self-serve");
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn a_completed_checkout_lifts_the_free_cap() {
	let stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	let capped = Terms {
		included_documents: Some(0),
		..Terms::of(Plan::Hobby)
	};
	fixture
		.store
		.set_terms(&fixture.project, &capped)
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

	let session = json!({ "id": "cs_1", "subscription": "sub_1" });
	let forged = reqwest::Client::new()
		.post(fixture.http("/v1/stripe/webhook"))
		.header("stripe-signature", stripe_signature("whsec_wrong", "{}"))
		.body("{}")
		.send()
		.await
		.expect("responds");
	assert_eq!(forged.status(), 400);

	stripe.put_subscription("sub_1", fixture.project.as_str(), "builder", "active");
	let response = deliver(&fixture, "checkout.session.completed", session).await;
	assert_eq!(response.status(), 200);
	assert!(
		client.wait_synced(),
		"the paused session syncs once the subscription pays"
	);

	let usage = usage(&fixture).await;
	assert_eq!(usage["plan"], "builder");
	assert_eq!(usage["subscription_status"], "active");

	let again = post(
		&fixture,
		"/v1/billing/checkout",
		json!({ "plan": "scale", "success_url": "https://a.example", "cancel_url": "https://a.example" }),
	)
	.await;
	assert_eq!(
		again.status(),
		409,
		"a paying project switches plans instead"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn webhooks_apply_the_subscription_as_it_is_now() {
	let stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	let project = fixture.project.as_str();
	stripe.put_subscription("sub_1", project, "builder", "active");

	// A late `created` event still carries the incomplete snapshot; the
	// server reads the subscription as it is now.
	let stale = json!({ "id": "sub_1", "status": "incomplete" });
	deliver(&fixture, "customer.subscription.created", stale).await;
	assert_eq!(usage(&fixture).await["plan"], "builder");

	// Cancelling an older subscription cannot downgrade the current one.
	stripe.put_subscription("sub_old", project, "scale", "canceled");
	deliver(
		&fixture,
		"customer.subscription.deleted",
		json!({ "id": "sub_old" }),
	)
	.await;
	assert_eq!(usage(&fixture).await["plan"], "builder");

	// A failed payment keeps the plan while Stripe retries.
	stripe.put_subscription("sub_1", project, "builder", "past_due");
	let invoice = json!({ "parent": { "subscription_details": { "subscription": "sub_1" } } });
	deliver(&fixture, "invoice.payment_failed", invoice.clone()).await;
	let usage_now = usage(&fixture).await;
	assert_eq!(usage_now["plan"], "builder");
	assert_eq!(usage_now["subscription_status"], "past_due");

	// Retries ran out: back to Hobby, but documents never go read-only.
	write(&fixture, "kept").await;
	stripe.put_subscription("sub_1", project, "builder", "unpaid");
	deliver(
		&fixture,
		"customer.subscription.updated",
		json!({ "id": "sub_1" }),
	)
	.await;
	assert_eq!(usage(&fixture).await["plan"], "hobby");
	write(&fixture, "kept").await;

	// Paying the open invoice restores the plan.
	stripe.put_subscription("sub_1", project, "builder", "active");
	deliver(&fixture, "invoice.paid", invoice).await;
	assert_eq!(usage(&fixture).await["plan"], "builder");

	let ignored = deliver(&fixture, "charge.refunded", json!({ "id": "ch_1" })).await;
	assert_eq!(ignored.status(), 200);
	assert_eq!(
		ignored.json::<Json>().await.expect("json")["handled"],
		false
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn plans_switch_between_builder_and_scale() {
	let stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	let refused = post(&fixture, "/v1/billing/plan", json!({ "plan": "scale" })).await;
	assert_eq!(refused.status(), 409, "only a paying project can switch");

	stripe.put_subscription("sub_1", fixture.project.as_str(), "builder", "active");
	deliver(
		&fixture,
		"customer.subscription.created",
		json!({ "id": "sub_1" }),
	)
	.await;
	let response = post(&fixture, "/v1/billing/plan", json!({ "plan": "scale" })).await;
	assert_eq!(response.status(), 200);
	let body: Json = response.json().await.expect("json");
	assert_eq!(body["terms"]["plan"], "scale");
	assert_eq!(body["terms"]["included_documents"], 150_000);

	let [update] = stripe
		.posts("/v1/subscriptions/sub_1")
		.try_into()
		.expect("one update");
	assert_eq!(
		update.field("proration_behavior"),
		Some("create_prorations")
	);
	assert_eq!(update.field("items[0][id]"), Some("si_sub_1_0"));
	assert_eq!(
		update.field("items[0][price]"),
		Some("price_canvist_scale_monthly")
	);
	assert_eq!(
		update.field("items[1][price]"),
		Some("price_canvist_scale_mad_overage")
	);
	assert_eq!(
		update.field("items[2][price]"),
		Some("price_canvist_scale_conversion_overage")
	);
	assert_eq!(usage(&fixture).await["plan"], "scale");
}

#[tokio::test(flavor = "multi_thread")]
async fn annual_checkout_saves_a_card_for_the_subscription() {
	let stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	let project = fixture.project.as_str();
	let request = json!({
		"plan": "scale",
		"interval": "year",
		"success_url": "https://app.example/billing/done",
		"cancel_url": "https://app.example/billing",
	});
	let response = post(&fixture, "/v1/billing/checkout", request.clone()).await;
	assert_eq!(response.status(), 200);
	let body: Json = response.json().await.expect("json");
	assert_eq!(body["url"], "https://checkout.stripe.test/cs_test_1");

	let [session] = stripe
		.posts("/v1/checkout/sessions")
		.try_into()
		.expect("one session");
	assert_eq!(
		session.field("mode"),
		Some("setup"),
		"Checkout cannot create a mixed-interval subscription"
	);
	assert_eq!(session.field("currency"), Some("usd"));
	assert_eq!(session.field("metadata[canvist_project]"), Some(project));
	assert_eq!(session.field("metadata[canvist_plan]"), Some("scale"));
	assert_eq!(session.field("client_reference_id"), Some(project));
	assert_eq!(session.field("tax_id_collection[enabled]"), Some("true"));
	assert_eq!(session.field("customer_creation"), Some("always"));
	assert_eq!(session.field("customer"), None);
	assert_eq!(session.field("line_items[0][price]"), None);
	assert_eq!(
		session.field("subscription_data[billing_cycle_anchor]"),
		None
	);

	// A returning customer keeps their customer.
	let returning = Terms {
		stripe_customer: Some("cus_test".to_owned()),
		..Terms::of(Plan::Hobby)
	};
	fixture
		.store
		.set_terms(&fixture.project, &returning)
		.await
		.expect("sets terms");
	let response = post(&fixture, "/v1/billing/checkout", request).await;
	assert_eq!(response.status(), 200);
	let sessions = stripe.posts("/v1/checkout/sessions");
	let session = sessions.last().expect("a second session");
	assert_eq!(session.field("customer"), Some("cus_test"));
	assert_eq!(session.field("customer_update[address]"), Some("auto"));
	assert_eq!(session.field("customer_creation"), None);

	let weekly = post(
		&fixture,
		"/v1/billing/checkout",
		json!({ "plan": "scale", "interval": "week", "success_url": "https://a.example", "cancel_url": "https://a.example" }),
	)
	.await;
	assert!(weekly.status().is_client_error(), "{}", weekly.status());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_completed_annual_checkout_starts_one_subscription() {
	let stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	let project = fixture.project.as_str();
	let completed = |id: &str| json!({ "id": id, "object": "checkout.session", "mode": "setup" });

	// The customer left before saving a card.
	stripe.put_annual_checkout_session("cs_open", project, "scale", "open");
	let response = deliver(&fixture, "checkout.session.completed", completed("cs_open")).await;
	assert_eq!(response.status(), 200);
	assert_eq!(
		response.json::<Json>().await.expect("json")["handled"],
		false
	);
	assert!(stripe.posts("/v1/subscriptions").is_empty());

	stripe.put_annual_checkout_session("cs_annual", project, "scale", "complete");
	let response = deliver(
		&fixture,
		"checkout.session.completed",
		completed("cs_annual"),
	)
	.await;
	assert_eq!(response.status(), 200);
	assert_eq!(
		response.json::<Json>().await.expect("json")["handled"],
		true
	);

	let [created] = stripe
		.posts("/v1/subscriptions")
		.try_into()
		.expect("one subscription");
	let anchor = Month::current().next().start_unix_seconds().to_string();
	assert_eq!(
		created.headers["idempotency-key"], "canvist-annual-cs_annual",
		"a retried webhook cannot create a second subscription"
	);
	assert_eq!(created.field("customer"), Some("cus_test"));
	assert_eq!(
		created.field("default_payment_method"),
		Some("pm_card_visa")
	);
	assert_eq!(created.field("billing_mode[type]"), Some("flexible"));
	assert_eq!(created.field("billing_cycle_anchor"), Some(anchor.as_str()));
	assert_eq!(created.field("off_session"), Some("true"));
	assert_eq!(created.field("metadata[canvist_project]"), Some(project));
	assert_eq!(
		created.field("metadata[canvist_checkout_session]"),
		Some("cs_annual")
	);
	assert_eq!(
		created.field("items[0][price]"),
		Some("price_canvist_scale_annual")
	);
	assert_eq!(created.field("items[0][quantity]"), Some("1"));
	assert_eq!(
		created.field("items[1][price]"),
		Some("price_canvist_scale_mad_overage")
	);
	assert_eq!(created.field("items[1][quantity]"), None);
	assert_eq!(
		created.field("items[2][price]"),
		Some("price_canvist_scale_conversion_overage")
	);
	assert_eq!(created.field("automatic_tax[enabled]"), None);

	let usage_now = usage(&fixture).await;
	assert_eq!(usage_now["plan"], "scale");
	assert_eq!(usage_now["included_documents"], 150_000);
	assert_eq!(usage_now["subscription_status"], "active");

	// Stripe retries and duplicates find the subscription instead of making
	// another, even after Stripe forgets the idempotency key.
	for _ in 0..2 {
		let again = deliver(
			&fixture,
			"checkout.session.completed",
			completed("cs_annual"),
		)
		.await;
		assert_eq!(again.status(), 200);
		assert_eq!(again.json::<Json>().await.expect("json")["handled"], true);
	}

	assert_eq!(stripe.posts("/v1/subscriptions").len(), 1);

	// A second annual checkout for a project that already pays saves a card
	// but never bills twice.
	stripe.put_annual_checkout_session("cs_second", project, "builder", "complete");
	let second = deliver(
		&fixture,
		"checkout.session.completed",
		completed("cs_second"),
	)
	.await;
	assert_eq!(second.status(), 200);
	assert_eq!(second.json::<Json>().await.expect("json")["handled"], false);
	assert_eq!(stripe.posts("/v1/subscriptions").len(), 1);
	assert_eq!(usage(&fixture).await["plan"], "scale");
}

#[tokio::test(flavor = "multi_thread")]
async fn annual_subscriptions_stay_annual_when_switching_plans() {
	let stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	stripe.put_subscription_billed(
		"sub_1",
		fixture.project.as_str(),
		"builder",
		"annual",
		"active",
	);
	deliver(
		&fixture,
		"customer.subscription.created",
		json!({ "id": "sub_1" }),
	)
	.await;
	assert_eq!(usage(&fixture).await["plan"], "builder");

	let response = post(&fixture, "/v1/billing/plan", json!({ "plan": "scale" })).await;
	assert_eq!(response.status(), 200);
	let [update] = stripe
		.posts("/v1/subscriptions/sub_1")
		.try_into()
		.expect("one update");
	assert_eq!(update.field("items[0][id]"), Some("si_sub_1_0"));
	assert_eq!(
		update.field("items[0][price]"),
		Some("price_canvist_scale_annual")
	);
	assert_eq!(
		update.field("items[1][price]"),
		Some("price_canvist_scale_mad_overage")
	);
	assert_eq!(usage(&fixture).await["plan"], "scale");
	let switched = stripe.subscription("sub_1").expect("still exists");
	assert_eq!(
		switched["items"]["data"][0]["price"]["recurring"]["interval"],
		"year"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_portal_opens_for_projects_with_a_customer() {
	let stripe = FakeStripe::start().await;
	let fixture = start(&stripe).await;
	let request = json!({ "return_url": "https://app.example/billing" });
	let refused = post(&fixture, "/v1/billing/portal", request.clone()).await;
	assert_eq!(refused.status(), 409);

	on_builder(&fixture).await;
	let response = post(&fixture, "/v1/billing/portal", request).await;
	assert_eq!(response.status(), 200);
	let body: Json = response.json().await.expect("json");
	assert_eq!(body["url"], "https://billing.stripe.test/session");

	let [session] = stripe
		.posts("/v1/billing_portal/sessions")
		.try_into()
		.expect("one session");
	assert_eq!(session.field("customer"), Some("cus_test"));
	assert_eq!(session.field("configuration"), Some("bpc_canvist"));
	assert_eq!(
		session.field("return_url"),
		Some("https://app.example/billing")
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn billing_is_off_without_stripe() {
	let fixture = Fixture::start().await;

	for (path, body) in [
		(
			"/v1/billing/checkout",
			json!({ "plan": "builder", "success_url": "https://a.example", "cancel_url": "https://a.example" }),
		),
		(
			"/v1/billing/portal",
			json!({ "return_url": "https://a.example" }),
		),
		("/v1/billing/plan", json!({ "plan": "scale" })),
	] {
		assert_eq!(post(&fixture, path, body).await.status(), 404, "{path}");
	}

	let webhook = reqwest::Client::new()
		.post(fixture.http("/v1/stripe/webhook"))
		.body("{}")
		.send()
		.await
		.expect("responds");
	assert_eq!(webhook.status(), 404);
}
