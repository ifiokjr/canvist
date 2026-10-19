//! Debounced `document.updated` webhooks.
//!
//! Rooms collect changes; the flush loop hands them here once a document has
//! been quiet for the debounce period, or has kept changing for the maximum
//! wait. Each delivery is a signed JSON `POST`, retried three times with
//! backoff and then dropped: webhooks are a hint to re-read the document,
//! not a log of every change.
//!
//! The body is signed like Stripe's: the `Canvist-Signature` header is
//! `t=<unix seconds>,v1=<hex HMAC-SHA256 of "<t>.<body>">`, keyed by the
//! webhook secret.

use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use hmac::Hmac;
use hmac::Mac;
use serde::Serialize;
use sha2::Sha256;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::room::Changes;
use crate::names::DocumentKey;
use crate::names::random_token;
use crate::store::Store;

/// Deliveries waiting before new ones are dropped.
const QUEUE: usize = 4096;
/// Deliveries in flight at once.
const CONCURRENCY: usize = 32;
/// Attempts per delivery.
const ATTEMPTS: u32 = 3;
const TIMEOUT: Duration = Duration::from_secs(10);
/// Random characters in an event id: 80 bits.
const EVENT_ID_LENGTH: usize = 16;

/// Queues webhook deliveries.
#[derive(Clone, Debug)]
pub struct Webhooks {
	sender: mpsc::Sender<Event>,
}

#[derive(Debug)]
struct Event {
	key: DocumentKey,
	payload: Payload,
}

/// The JSON body of a `document.updated` webhook.
#[derive(Clone, Debug, Serialize)]
struct Payload {
	#[serde(rename = "type")]
	kind: &'static str,
	id: String,
	project: String,
	environment: String,
	document: String,
	editors: Vec<String>,
	first_change_at: u64,
	last_change_at: u64,
}

/// The webhook worker, to wait for on shutdown.
#[derive(Debug)]
pub struct WebhookWorker {
	handle: JoinHandle<()>,
	stop: watch::Sender<bool>,
}

impl WebhookWorker {
	/// Stop accepting deliveries and wait up to `deadline` for the queued
	/// ones to finish.
	pub async fn drain(self, deadline: Duration) {
		let _ = self.stop.send(true);

		if tokio::time::timeout(deadline, self.handle).await.is_err() {
			tracing::warn!("webhook deliveries still running at shutdown were dropped");
		}
	}
}

impl Webhooks {
	/// Start the delivery worker.
	pub fn start(store: Store) -> (Self, WebhookWorker) {
		let (sender, receiver) = mpsc::channel(QUEUE);
		let client = reqwest::Client::builder()
			.timeout(TIMEOUT)
			.redirect(reqwest::redirect::Policy::none())
			.user_agent("canvist-cloud-webhooks")
			.build()
			.expect("the webhook client configuration is valid");
		let (stop, stopped) = watch::channel(false);
		let handle = tokio::spawn(run(store, client, receiver, stopped));
		(Self { sender }, WebhookWorker { handle, stop })
	}

	/// Queue a webhook for `changes` to `key`, if its project environment
	/// has one configured.
	pub fn notify(&self, key: &DocumentKey, changes: &Changes) {
		let now = Instant::now();
		let wall_clock =
			|instant: Instant| unix_millis(SystemTime::now() - now.duration_since(instant));
		let event = Event {
			key: key.clone(),
			payload: Payload {
				kind: "document.updated",
				id: format!("evt_{}", random_token(EVENT_ID_LENGTH)),
				project: key.project.to_string(),
				environment: key.environment.to_string(),
				document: key.name.to_string(),
				editors: changes.editors.iter().map(ToString::to_string).collect(),
				first_change_at: wall_clock(changes.first),
				last_change_at: wall_clock(changes.last),
			},
		};

		if self.sender.try_send(event).is_err() {
			tracing::warn!(document = %key, "webhook queue full; dropping a delivery");
		}
	}
}

/// Receive events until stopped, delivering up to [`CONCURRENCY`] at once.
/// Once stopped, the queue is closed but drained, and in-flight deliveries
/// finish, so shutdown loses nothing that was already queued.
async fn run(
	store: Store,
	client: reqwest::Client,
	mut receiver: mpsc::Receiver<Event>,
	mut stopped: watch::Receiver<bool>,
) {
	let permits = Arc::new(Semaphore::new(CONCURRENCY));
	let mut deliveries = tokio::task::JoinSet::new();
	let mut stopping = false;

	loop {
		let event = tokio::select! {
			event = receiver.recv() => event,
			_ = stopped.changed(), if !stopping => {
				stopping = true;
				receiver.close();
				continue;
			}
		};
		let Some(event) = event else {
			break;
		};
		let Ok(permit) = permits.clone().acquire_owned().await else {
			break;
		};
		let store = store.clone();
		let client = client.clone();

		deliveries.spawn(async move {
			deliver(&store, &client, event).await;
			drop(permit);
		});

		while deliveries.try_join_next().is_some() {}
	}

	while deliveries.join_next().await.is_some() {}
}

/// Deliver `event` to its environment's webhook, if one is configured.
async fn deliver(store: &Store, client: &reqwest::Client, event: Event) {
	let configured = store
		.webhook(&event.key.project, &event.key.environment)
		.await;
	let webhook = match configured {
		Ok(Some(webhook)) => webhook,
		Ok(None) => return,
		Err(error) => {
			tracing::warn!(document = %event.key, %error, "could not read the webhook configuration");
			return;
		}
	};
	let body = serde_json::to_string(&event.payload).expect("payloads always serialize");

	for attempt in 0..ATTEMPTS {
		if attempt > 0 {
			tokio::time::sleep(Duration::from_secs(4_u64.pow(attempt - 1))).await;
		}

		if attempt_delivery(client, &webhook, &event, &body, attempt).await {
			return;
		}
	}

	tracing::warn!(document = %event.key, "webhook dropped after {ATTEMPTS} attempts");
}

/// One signed `POST`; returns whether the endpoint accepted it.
async fn attempt_delivery(
	client: &reqwest::Client,
	webhook: &crate::store::Webhook,
	event: &Event,
	body: &str,
	attempt: u32,
) -> bool {
	let timestamp = unix_millis(SystemTime::now()) / 1000;
	let response = client
		.post(&webhook.url)
		.header("content-type", "application/json")
		.header("canvist-event", event.payload.kind)
		.header("canvist-signature", sign(&webhook.secret, timestamp, body))
		.body(body.to_owned())
		.send()
		.await;

	match response {
		Ok(response) if response.status().is_success() => true,
		Ok(response) => {
			tracing::info!(document = %event.key, status = %response.status(), attempt, "webhook refused");
			false
		}
		Err(error) => {
			tracing::info!(document = %event.key, %error, attempt, "webhook failed");
			false
		}
	}
}

/// The `Canvist-Signature` header for `body` sent at `timestamp`.
pub fn sign(secret: &str, timestamp: u64, body: &str) -> String {
	let mut mac =
		Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
	mac.update(format!("{timestamp}.{body}").as_bytes());
	format!("t={timestamp},v1={}", hex(&mac.finalize().into_bytes()))
}

/// Lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
	use std::fmt::Write;

	bytes.iter().fold(String::new(), |mut out, byte| {
		let _ = write!(out, "{byte:02x}");
		out
	})
}

fn unix_millis(time: SystemTime) -> u64 {
	time.duration_since(UNIX_EPOCH)
		.map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// Why a webhook URL was refused.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid webhook url: {0}")]
pub struct InvalidUrl(&'static str);

/// Check that `url` is an HTTPS URL on a public host. With `allow_private`
/// (for tests and self-hosting), plain HTTP and private hosts are allowed.
///
/// This blocks the obvious ways to aim webhooks at the relay's own network.
/// A public name that resolves to a private address is not caught here.
pub fn validate_url(url: &str, allow_private: bool) -> Result<(), InvalidUrl> {
	let parsed = reqwest::Url::parse(url).map_err(|_| InvalidUrl("not a URL"))?;

	match (parsed.scheme(), allow_private) {
		("http" | "https", true) => return Ok(()),
		(_, true) => return Err(InvalidUrl("use http or https")),
		("https", false) => {}
		(_, false) => return Err(InvalidUrl("use https")),
	}

	let host = parsed.host_str().ok_or(InvalidUrl("missing host"))?;
	check_public_host(host.trim_start_matches('[').trim_end_matches(']'))
}

/// Refuse loopback, private, and link-local addresses and internal names.
fn check_public_host(host: &str) -> Result<(), InvalidUrl> {
	if let Ok(ip) = host.parse::<IpAddr>() {
		return if is_public(ip) {
			Ok(())
		} else {
			Err(InvalidUrl("private address"))
		};
	}

	let private_name =
		host == "localhost" || PRIVATE_SUFFIXES.iter().any(|suffix| host.ends_with(suffix));

	if private_name {
		Err(InvalidUrl("private host"))
	} else {
		Ok(())
	}
}

/// Name suffixes that only resolve inside a private network; Railway's
/// private network is `*.railway.internal`.
const PRIVATE_SUFFIXES: [&str; 3] = [".localhost", ".internal", ".local"];

fn is_public(ip: IpAddr) -> bool {
	match ip {
		IpAddr::V4(ip) => is_public_v4(ip),
		IpAddr::V6(ip) => is_public_v6(ip),
	}
}

/// Not private, loopback, link-local, unspecified, broadcast, or carrier
/// NAT (`100.64.0.0/10`).
fn is_public_v4(ip: Ipv4Addr) -> bool {
	let [first, second, ..] = ip.octets();
	let carrier_nat = first == 100 && (second & 0xc0) == 0x40;

	!(ip.is_private()
		|| ip.is_loopback()
		|| ip.is_link_local()
		|| ip.is_unspecified()
		|| ip.is_broadcast()
		|| carrier_nat)
}

/// Not loopback, unspecified, unique-local (`fc00::/7`), link-local
/// (`fe80::/10`), or an IPv4-mapped private address.
fn is_public_v6(ip: Ipv6Addr) -> bool {
	let first = ip.segments()[0];
	let unique_local = (first & 0xfe00) == 0xfc00;
	let link_local = (first & 0xffc0) == 0xfe80;
	let mapped_private = ip.to_ipv4_mapped().is_some_and(|v4| !is_public_v4(v4));

	!(ip.is_loopback() || ip.is_unspecified() || unique_local || link_local || mapped_private)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn signatures_are_stable() {
		assert_eq!(
			sign("secret", 1_700_000_000, "{}"),
			sign("secret", 1_700_000_000, "{}")
		);
		assert_ne!(
			sign("secret", 1_700_000_000, "{}"),
			sign("other", 1_700_000_000, "{}")
		);
		assert!(sign("secret", 1, "{}").starts_with("t=1,v1="));
	}

	#[test]
	fn private_targets_are_refused() {
		assert!(validate_url("https://example.com/hook", false).is_ok());
		assert!(validate_url("http://example.com/hook", false).is_err());
		assert!(validate_url("https://127.0.0.1/hook", false).is_err());
		assert!(validate_url("https://10.1.2.3/hook", false).is_err());
		assert!(validate_url("https://[::1]/hook", false).is_err());
		assert!(validate_url("https://postgres.railway.internal/", false).is_err());
		assert!(validate_url("https://localhost/", false).is_err());
		assert!(validate_url("http://127.0.0.1:9000/hook", true).is_ok());
		assert!(validate_url("ftp://example.com", true).is_err());
	}
}
