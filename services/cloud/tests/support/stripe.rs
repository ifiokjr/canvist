//! A stand-in for the Stripe API: the canvist catalog, subscriptions the
//! test controls, and a record of every request.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::RawQuery;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use canvist_cloud::config::Secret;
use canvist_cloud::config::StripeConfig;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

/// A request the fake received.
#[derive(Clone, Debug)]
pub struct Recorded {
	pub path: String,
	pub headers: HeaderMap,
	pub form: Vec<(String, String)>,
}

impl Recorded {
	/// The first value of form field `name`.
	pub fn field(&self, name: &str) -> Option<&str> {
		self.form
			.iter()
			.find(|(key, _)| key == name)
			.map(|(_, value)| value.as_str())
	}
}

#[derive(Default)]
struct Inner {
	subscriptions: HashMap<String, Value>,
	failing_meter_events: usize,
	posts: Vec<Recorded>,
}

#[derive(Clone)]
struct Shared {
	inner: Arc<Mutex<Inner>>,
	meter_events: mpsc::UnboundedSender<Recorded>,
}

/// A running fake.
pub struct FakeStripe {
	pub url: String,
	shared: Shared,
	meter_events: mpsc::UnboundedReceiver<Recorded>,
}

/// The lookup keys of the catalog's prices, and whether each is metered.
const PRICES: [(&str, bool); 6] = [
	("canvist_builder_monthly", false),
	("canvist_builder_mad_overage", true),
	("canvist_builder_conversion_overage", true),
	("canvist_scale_monthly", false),
	("canvist_scale_mad_overage", true),
	("canvist_scale_conversion_overage", true),
];

impl FakeStripe {
	pub async fn start() -> Self {
		let (sender, meter_events) = mpsc::unbounded_channel();
		let shared = Shared {
			inner: Arc::default(),
			meter_events: sender,
		};
		let app = Router::new()
			.route("/v1/billing/meter_events", post(meter_event))
			.route("/v1/prices", get(prices))
			.route("/v1/checkout/sessions", post(checkout_session))
			.route(
				"/v1/subscriptions/{id}",
				get(subscription).post(update_subscription),
			)
			.route(
				"/v1/billing_portal/configurations",
				get(portal_configurations),
			)
			.route("/v1/billing_portal/sessions", post(portal_session))
			.with_state(shared.clone());
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
			.await
			.expect("binds");
		let url = format!("http://{}", listener.local_addr().expect("addr"));
		tokio::spawn(async move { axum::serve(listener, app).await });

		Self {
			url,
			shared,
			meter_events,
		}
	}

	/// Billing configuration pointing at this fake, reporting quickly.
	pub fn config(&self) -> StripeConfig {
		StripeConfig {
			secret_key: Secret::new("sk_test_canvist"),
			meter_event: "canvist_active_document".to_owned(),
			conversion_meter_event: "canvist_conversion".to_owned(),
			api_base: self.url.clone(),
			report_interval: Duration::from_millis(50),
			automatic_tax: false,
		}
	}

	/// Store a subscription for `project` on `plan` with `status`.
	pub fn put_subscription(&self, id: &str, project: &str, plan: &str, status: &str) {
		let items: Vec<Value> = ["monthly", "mad_overage", "conversion_overage"]
			.iter()
			.enumerate()
			.map(|(index, suffix)| {
				price_item(
					&format!("si_{id}_{index}"),
					&format!("canvist_{plan}_{suffix}"),
				)
			})
			.collect();
		let subscription = json!({
			"id": id,
			"object": "subscription",
			"customer": "cus_test",
			"status": status,
			"metadata": { "canvist_project": project },
			"items": { "data": items },
		});
		self.lock()
			.subscriptions
			.insert(id.to_owned(), subscription);
	}

	/// Answer the next `count` meter events with a server error.
	pub fn fail_meter_events(&self, count: usize) {
		self.lock().failing_meter_events = count;
	}

	/// The next meter event Stripe accepted, waiting up to five seconds.
	pub async fn next_meter_event(&mut self) -> Recorded {
		tokio::time::timeout(Duration::from_secs(5), self.meter_events.recv())
			.await
			.expect("a meter event arrives")
			.expect("the fake is running")
	}

	/// Whether no further meter event arrives within `wait`.
	pub async fn no_meter_event_within(&mut self, wait: Duration) -> bool {
		tokio::time::timeout(wait, self.meter_events.recv())
			.await
			.is_err()
	}

	/// Every POST to `path`, oldest first.
	pub fn posts(&self, path: &str) -> Vec<Recorded> {
		self.lock()
			.posts
			.iter()
			.filter(|recorded| recorded.path == path)
			.cloned()
			.collect()
	}

	fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
		self.shared.inner.lock().expect("not poisoned")
	}
}

fn price_item(id: &str, lookup_key: &str) -> Value {
	let metered = PRICES
		.iter()
		.any(|(key, metered)| *key == lookup_key && *metered);
	json!({
		"id": id,
		"price": price(lookup_key, metered),
	})
}

fn price(lookup_key: &str, metered: bool) -> Value {
	json!({
		"id": format!("price_{lookup_key}"),
		"lookup_key": lookup_key,
		"recurring": { "usage_type": if metered { "metered" } else { "licensed" } },
	})
}

fn record(shared: &Shared, path: &str, headers: HeaderMap, body: &str) -> Recorded {
	let recorded = Recorded {
		path: path.to_owned(),
		headers,
		form: decode_form(body),
	};
	shared
		.inner
		.lock()
		.expect("not poisoned")
		.posts
		.push(recorded.clone());
	recorded
}

async fn meter_event(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> (StatusCode, Json<Value>) {
	let recorded = record(&shared, "/v1/billing/meter_events", headers, &body);
	{
		let mut inner = shared.inner.lock().expect("not poisoned");

		if inner.failing_meter_events > 0 {
			inner.failing_meter_events -= 1;
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(json!({ "error": { "message": "try again" } })),
			);
		}
	}
	let _ = shared.meter_events.send(recorded);
	(
		StatusCode::OK,
		Json(json!({ "object": "billing.meter_event" })),
	)
}

async fn prices(RawQuery(query): RawQuery) -> Json<Value> {
	let wanted: Vec<String> = decode_form(&query.unwrap_or_default())
		.into_iter()
		.filter(|(key, _)| key == "lookup_keys[]")
		.map(|(_, value)| value)
		.collect();
	let data: Vec<Value> = PRICES
		.iter()
		.filter(|(key, _)| wanted.iter().any(|wanted| wanted == key))
		.map(|(key, metered)| price(key, *metered))
		.collect();
	Json(json!({ "object": "list", "data": data }))
}

async fn checkout_session(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> Json<Value> {
	record(&shared, "/v1/checkout/sessions", headers, &body);
	Json(json!({ "id": "cs_test_1", "url": "https://checkout.stripe.test/cs_test_1" }))
}

async fn subscription(
	State(shared): State<Shared>,
	Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
	shared
		.inner
		.lock()
		.expect("not poisoned")
		.subscriptions
		.get(&id)
		.cloned()
		.map(Json)
		.ok_or(StatusCode::NOT_FOUND)
}

/// Swap item prices as `items[i][id]` and `items[i][price]` ask.
async fn update_subscription(
	State(shared): State<Shared>,
	Path(id): Path<String>,
	headers: HeaderMap,
	body: String,
) -> Result<Json<Value>, StatusCode> {
	let recorded = record(&shared, &format!("/v1/subscriptions/{id}"), headers, &body);
	let mut inner = shared.inner.lock().expect("not poisoned");
	let subscription = inner
		.subscriptions
		.get_mut(&id)
		.ok_or(StatusCode::NOT_FOUND)?;

	for index in 0.. {
		let (Some(item), Some(price)) = (
			recorded.field(&format!("items[{index}][id]")),
			recorded.field(&format!("items[{index}][price]")),
		) else {
			break;
		};
		let lookup_key = price
			.strip_prefix("price_")
			.ok_or(StatusCode::BAD_REQUEST)?;
		let items = subscription["items"]["data"]
			.as_array_mut()
			.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
		let entry = items
			.iter_mut()
			.find(|entry| entry["id"] == item)
			.ok_or(StatusCode::BAD_REQUEST)?;
		*entry = price_item(item, lookup_key);
	}

	Ok(Json(subscription.clone()))
}

async fn portal_configurations() -> Json<Value> {
	Json(json!({
		"object": "list",
		"data": [
			{ "id": "bpc_other", "metadata": {} },
			{ "id": "bpc_canvist", "metadata": { "canvist_key": "portal" } },
		],
	}))
}

async fn portal_session(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> Json<Value> {
	record(&shared, "/v1/billing_portal/sessions", headers, &body);
	Json(json!({ "url": "https://billing.stripe.test/session" }))
}

/// Decode `a=b&c=d` with percent escapes and `+` for spaces.
pub fn decode_form(text: &str) -> Vec<(String, String)> {
	text.split('&')
		.filter_map(|pair| pair.split_once('='))
		.map(|(key, value)| (decode(key), decode(value)))
		.collect()
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
