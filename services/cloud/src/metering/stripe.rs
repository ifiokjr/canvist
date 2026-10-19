//! Reporting new monthly active documents to Stripe Billing Meters.
//!
//! Each document that becomes active in a month on a project with a Stripe
//! customer sends one meter event with value 1. The meter sums them, so the
//! customer's invoice counts MADs. Events carry a deterministic
//! `identifier`, so a retry never counts a document twice.
//!
//! Only test-mode keys are accepted ([`StripeConfig`] refuses live ones), so
//! this cannot bill anyone until the owner deliberately wires live billing.

use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use sha2::Digest;
use sha2::Sha256;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::Month;
use crate::config::StripeConfig;
use crate::names::DocumentKey;

/// Events waiting before new ones are dropped.
const QUEUE: usize = 4096;
/// Attempts per event.
const ATTEMPTS: u32 = 3;
const TIMEOUT: Duration = Duration::from_secs(10);
/// Hex characters of the identifier hash; Stripe allows 100 in all.
const IDENTIFIER_HASH_LENGTH: usize = 48;

/// Queues meter events.
#[derive(Clone, Debug)]
pub struct StripeMeter {
	sender: mpsc::Sender<MeterEvent>,
}

/// One document's first activity in a month.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeterEvent {
	/// The Stripe customer.
	pub customer: String,
	/// Deduplicates retries: the same document and month always give the
	/// same identifier.
	pub identifier: String,
	/// When it happened, in Unix seconds.
	pub timestamp: u64,
}

/// The reporting worker, to wait for on shutdown.
#[derive(Debug)]
pub struct StripeWorker {
	handle: JoinHandle<()>,
	stop: watch::Sender<bool>,
}

impl StripeWorker {
	/// Send what is queued, waiting at most `deadline`.
	pub async fn drain(self, deadline: Duration) {
		let _ = self.stop.send(true);

		if tokio::time::timeout(deadline, self.handle).await.is_err() {
			tracing::warn!("Stripe meter events still queued at shutdown were dropped");
		}
	}
}

impl StripeMeter {
	/// Start reporting with `config`.
	pub fn start(config: StripeConfig) -> (Self, StripeWorker) {
		let (sender, receiver) = mpsc::channel(QUEUE);
		let (stop, stopped) = watch::channel(false);
		let client = reqwest::Client::builder()
			.timeout(TIMEOUT)
			.build()
			.expect("the Stripe client configuration is valid");
		let handle = tokio::spawn(run(config, client, receiver, stopped));

		(Self { sender }, StripeWorker { handle, stop })
	}

	/// Report that `key` became active in `month` for `customer`.
	pub fn record(&self, customer: &str, key: &DocumentKey, month: Month) {
		let event = MeterEvent {
			customer: customer.to_owned(),
			identifier: identifier(key, month),
			timestamp: SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.map_or(0, |elapsed| elapsed.as_secs()),
		};

		if self.sender.try_send(event).is_err() {
			tracing::error!(document = %key, "Stripe meter queue full; an active document went unreported");
		}
	}
}

/// `mad_` and a hash of the document and month.
fn identifier(key: &DocumentKey, month: Month) -> String {
	let digest = Sha256::digest(format!("{key}@{month}").as_bytes());
	let hex = crate::sync::webhooks::hex(&digest);

	format!("mad_{}", &hex[..IDENTIFIER_HASH_LENGTH])
}

/// Send events one at a time until stopped, then send what is queued.
async fn run(
	config: StripeConfig,
	client: reqwest::Client,
	mut receiver: mpsc::Receiver<MeterEvent>,
	mut stopped: watch::Receiver<bool>,
) {
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

		send(&config, &client, &event).await;
	}
}

async fn send(config: &StripeConfig, client: &reqwest::Client, event: &MeterEvent) {
	for attempt in 0..ATTEMPTS {
		if attempt > 0 {
			tokio::time::sleep(Duration::from_secs(u64::from(attempt))).await;
		}

		match post(config, client, event).await {
			Ok(()) => return,
			Err(problem) => {
				tracing::warn!(identifier = %event.identifier, attempt, %problem, "Stripe meter event failed");
			}
		}
	}

	tracing::error!(identifier = %event.identifier, "Stripe meter event dropped after {ATTEMPTS} attempts");
}

/// One `POST /v1/billing/meter_events`. A duplicate identifier means an
/// earlier attempt already landed, which counts as success.
async fn post(
	config: &StripeConfig,
	client: &reqwest::Client,
	event: &MeterEvent,
) -> Result<(), String> {
	let value = "1".to_owned();
	let timestamp = event.timestamp.to_string();
	let form = [
		("event_name", &config.meter_event),
		("payload[stripe_customer_id]", &event.customer),
		("payload[value]", &value),
		("identifier", &event.identifier),
		("timestamp", &timestamp),
	];
	let response = client
		.post(format!("{}/v1/billing/meter_events", config.api_base))
		.bearer_auth(config.secret_key.expose())
		.form(&form)
		.send()
		.await
		.map_err(|error| error.to_string())?;
	let status = response.status();

	if status.is_success() {
		return Ok(());
	}

	let body = response.text().await.unwrap_or_default();

	if body.contains("resource_already_exists") {
		return Ok(());
	}

	Err(format!("{status}: {body}"))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::names::DocumentName;
	use crate::names::Environment;
	use crate::names::ProjectId;

	#[test]
	fn identifiers_are_stable_per_document_and_month() {
		let key = DocumentKey {
			project: ProjectId::new("prj_a"),
			environment: Environment::parse("production").expect("valid"),
			name: DocumentName::parse("notes").expect("valid"),
		};
		let october: Month = "2026-10".parse().expect("valid");
		let november: Month = "2026-11".parse().expect("valid");

		assert_eq!(identifier(&key, october), identifier(&key, october));
		assert_ne!(identifier(&key, october), identifier(&key, november));
		assert!(identifier(&key, october).len() <= 100);
	}
}
