//! Talking to Stripe: a small REST client, and the reporter that sends
//! usage to Billing Meters.
//!
//! Usage is written to the store first and reported afterwards, so a
//! deploy, crash, or Stripe outage delays usage but never loses it:
//!
//! - Each document that becomes active in a month on a paid plan is one
//!   meter event with value 1 and the identifier `mad_<hash>` of the
//!   document and month.
//! - Conversions are reported as the change since the last report. The
//!   range is reserved in the store before it is sent, so a retry resends
//!   the same value under the same identifier.
//!
//! Stripe refuses a repeated identifier for at least 24 hours, so a report
//! that landed but was not marked is not counted twice when it is retried.
//!
//! Only test-mode keys are accepted ([`StripeConfig`] refuses live ones), so
//! this cannot bill anyone until the owner deliberately wires live billing.

use std::time::Duration;

use serde_json::Value as Json;
use sha2::Digest;
use sha2::Sha256;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::Month;
use crate::config::StripeConfig;
use crate::names::DocumentKey;
use crate::names::random_token;
use crate::store::PendingConversions;
use crate::store::PendingDocument;
use crate::store::Store;
use crate::store::StoreError;

/// The API version every request pins, so a change to the account's default
/// version cannot change what the server reads.
pub const API_VERSION: &str = "2026-08-26.dahlia";

const TIMEOUT: Duration = Duration::from_secs(10);
/// Hex characters of an identifier hash; Stripe allows 100 in all.
const IDENTIFIER_HASH_LENGTH: usize = 48;
/// Random characters in an idempotency key.
const IDEMPOTENCY_KEY_LENGTH: usize = 32;
/// Rows read from the store per report pass.
const BATCH: usize = 100;
/// Stripe accepts meter events up to 35 days old; older activations can
/// never be reported, so they are dropped with an error instead of retried
/// forever.
const MAX_EVENT_AGE_SECONDS: i64 = 34 * 24 * 60 * 60;

/// A failed Stripe request.
#[derive(Debug, thiserror::Error)]
pub enum StripeError {
	/// The request never got an answer.
	#[error("could not reach Stripe: {0}")]
	Transport(String),
	/// Stripe answered with an error.
	#[error("Stripe returned {status}: {message}")]
	Api {
		/// The HTTP status.
		status: u16,
		/// Stripe's `error.code`, when it has one.
		code: Option<String>,
		/// Stripe's `error.message`.
		message: String,
	},
}

/// A Stripe REST client: form-encoded requests, JSON responses.
#[derive(Clone, Debug)]
pub struct StripeClient {
	http: reqwest::Client,
	config: StripeConfig,
}

impl StripeClient {
	/// A client for `config`.
	pub fn new(config: StripeConfig) -> Self {
		let http = reqwest::Client::builder()
			.timeout(TIMEOUT)
			.build()
			.expect("the Stripe client configuration is valid");

		Self { http, config }
	}

	/// The configuration.
	pub fn config(&self) -> &StripeConfig {
		&self.config
	}

	/// `GET path` with `query`.
	pub async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Json, StripeError> {
		let request = self
			.http
			.get(format!("{}{path}", self.config.api_base))
			.query(query);
		self.send(request).await
	}

	/// `POST path` with `form`. One idempotency key covers the request and
	/// its single retry after a transport failure, so a request that reached
	/// Stripe is not applied twice.
	pub async fn post(&self, path: &str, form: &[(String, String)]) -> Result<Json, StripeError> {
		self.post_once(path, form, &random_token(IDEMPOTENCY_KEY_LENGTH))
			.await
	}

	/// `POST path` with `form` under the caller's idempotency `key`, so
	/// separate attempts at the same request (a retried webhook) are applied
	/// once. Stripe remembers a key for at least 24 hours.
	pub async fn post_once(
		&self,
		path: &str,
		form: &[(String, String)],
		key: &str,
	) -> Result<Json, StripeError> {
		let request = || {
			self.http
				.post(format!("{}{path}", self.config.api_base))
				.header("idempotency-key", key)
				.form(form)
		};

		match self.send(request()).await {
			Err(StripeError::Transport(problem)) => {
				tracing::warn!(path, %problem, "retrying a Stripe request");
				self.send(request()).await
			}
			result => result,
		}
	}

	async fn send(&self, request: reqwest::RequestBuilder) -> Result<Json, StripeError> {
		let response = request
			.bearer_auth(self.config.secret_key.expose())
			.header("stripe-version", API_VERSION)
			.send()
			.await
			.map_err(|error| StripeError::Transport(error.to_string()))?;
		let status = response.status();
		let body: Json = response
			.json()
			.await
			.map_err(|error| StripeError::Transport(error.to_string()))?;

		if status.is_success() {
			return Ok(body);
		}

		let error = body.get("error");
		let text = |field: &str| {
			error
				.and_then(|error| error.get(field))
				.and_then(Json::as_str)
				.map(str::to_owned)
		};

		Err(StripeError::Api {
			status: status.as_u16(),
			code: text("code"),
			message: text("message").unwrap_or_default(),
		})
	}

	/// Send one meter event. A repeated identifier means an earlier attempt
	/// landed, which counts as success.
	async fn meter_event(&self, event: &MeterEvent<'_>) -> Result<(), StripeError> {
		let form = [
			("event_name", event.event_name.to_owned()),
			("payload[stripe_customer_id]", event.customer.to_owned()),
			("payload[value]", event.value.to_string()),
			("identifier", event.identifier.clone()),
			("timestamp", event.timestamp.to_string()),
		]
		.map(|(name, value)| (name.to_owned(), value));

		match self.post("/v1/billing/meter_events", &form).await {
			Err(StripeError::Api { message, .. }) if is_duplicate(&message) => Ok(()),
			result => result.map(drop),
		}
	}
}

/// Whether a meter event error says the identifier was already used. Stripe
/// answers a repeat with a `400` carrying no error code, only the message
/// "An event already exists with identifier …" (checked in test mode on
/// 2026-10-19).
fn is_duplicate(message: &str) -> bool {
	message.contains("already exists with identifier")
}

/// One meter event.
#[derive(Debug, PartialEq, Eq)]
struct MeterEvent<'a> {
	event_name: &'a str,
	customer: &'a str,
	value: u64,
	identifier: String,
	timestamp: i64,
}

/// `mad_` and a hash of the document and month.
fn document_identifier(key: &DocumentKey, month: Month) -> String {
	hashed("mad", &format!("{key}@{month}"))
}

/// `conv_` and a hash of the project environment, month, and range.
fn conversions_identifier(pending: &PendingConversions) -> String {
	hashed(
		"conv",
		&format!(
			"{}/{}@{}:{}-{}",
			pending.project, pending.environment, pending.month, pending.from, pending.to
		),
	)
}

fn hashed(prefix: &str, text: &str) -> String {
	let hex = crate::sync::webhooks::hex(&Sha256::digest(text.as_bytes()));
	format!("{prefix}_{}", &hex[..IDENTIFIER_HASH_LENGTH])
}

/// Reports what the store holds unreported, every `report_interval`.
#[derive(Debug)]
pub struct UsageReporter {
	client: StripeClient,
	store: Store,
}

/// The reporter's task, to stop at shutdown.
#[derive(Debug)]
pub struct ReporterWorker {
	handle: JoinHandle<()>,
	stop: watch::Sender<bool>,
}

impl ReporterWorker {
	/// Run one last report pass, waiting at most `deadline`. What it does not
	/// send stays in the store for the next start.
	pub async fn drain(self, deadline: Duration) {
		let _ = self.stop.send(true);

		if tokio::time::timeout(deadline, self.handle).await.is_err() {
			tracing::warn!("usage still unreported at shutdown; it is sent after the next start");
		}
	}
}

impl UsageReporter {
	/// Start reporting `store`'s usage through `client`.
	pub fn start(client: StripeClient, store: Store) -> ReporterWorker {
		let (stop, stopped) = watch::channel(false);
		let reporter = Self { client, store };
		let handle = tokio::spawn(reporter.run(stopped));

		ReporterWorker { handle, stop }
	}

	async fn run(self, mut stopped: watch::Receiver<bool>) {
		let mut interval = tokio::time::interval(self.client.config.report_interval);
		interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

		loop {
			tokio::select! {
				_ = interval.tick() => {}
				_ = stopped.changed() => {
					self.report().await;
					return;
				}
			}

			self.report().await;
		}
	}

	/// One pass over everything unreported. A Stripe failure ends the pass;
	/// the next one retries.
	async fn report(&self) {
		if let Err(error) = self.report_documents().await {
			tracing::warn!(%error, "could not report active documents");
		}

		if let Err(error) = self.report_conversions().await {
			tracing::warn!(%error, "could not report conversions");
		}
	}

	async fn report_documents(&self) -> Result<(), ReportError> {
		let now = crate::auth::unix_now() as i64;

		for pending in self.store.pending_documents(BATCH).await? {
			if now - pending.first_active_at > MAX_EVENT_AGE_SECONDS {
				tracing::error!(document = %pending.key, month = %pending.month, "an active document is too old to report to Stripe; dropping it");
			} else {
				let event = document_event(&self.client.config, &pending, now);
				send_or_drop(&self.client, &event).await?;
			}

			self.store
				.mark_document_reported(&pending.key, pending.month)
				.await?;
		}

		Ok(())
	}

	async fn report_conversions(&self) -> Result<(), ReportError> {
		let now = crate::auth::unix_now() as i64;

		for pending in self.store.reserve_conversions(BATCH).await? {
			if let Some(event) = conversions_event(&self.client.config, &pending, now) {
				send_or_drop(&self.client, &event).await?;
			}

			self.store.mark_conversions_reported(&pending).await?;
		}

		Ok(())
	}
}

/// Send `event`. An event Stripe refuses outright (an unknown customer, a
/// malformed value) is logged and dropped, so one bad row cannot hold up
/// everyone else's usage; any other failure is returned to retry later.
async fn send_or_drop(client: &StripeClient, event: &MeterEvent<'_>) -> Result<(), StripeError> {
	match client.meter_event(event).await {
		Err(
			error @ StripeError::Api {
				status: 400 | 404, ..
			},
		) => {
			tracing::error!(identifier = %event.identifier, customer = event.customer, %error, "Stripe refused a meter event; dropping it");
			Ok(())
		}
		result => result,
	}
}

/// The meter event for a document's activation.
fn document_event<'a>(
	config: &'a StripeConfig,
	pending: &'a PendingDocument,
	now: i64,
) -> MeterEvent<'a> {
	MeterEvent {
		event_name: &config.meter_event,
		customer: &pending.customer,
		value: 1,
		identifier: document_identifier(&pending.key, pending.month),
		timestamp: pending.first_active_at.min(now),
	}
}

/// The meter event for reserved conversions, timestamped inside their month
/// so they land in that month's billing period. `None` when there is
/// nothing to send.
fn conversions_event<'a>(
	config: &'a StripeConfig,
	pending: &'a PendingConversions,
	now: i64,
) -> Option<MeterEvent<'a>> {
	let value = pending
		.to
		.checked_sub(pending.from)
		.filter(|value| *value > 0)?;

	Some(MeterEvent {
		event_name: &config.conversion_meter_event,
		customer: &pending.customer,
		value,
		identifier: conversions_identifier(pending),
		timestamp: now.min(pending.month.end_unix_seconds()),
	})
}

#[derive(Debug, thiserror::Error)]
enum ReportError {
	#[error(transparent)]
	Store(#[from] StoreError),
	#[error(transparent)]
	Stripe(#[from] StripeError),
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::Secret;
	use crate::names::DocumentName;
	use crate::names::Environment;
	use crate::names::ProjectId;

	fn key() -> DocumentKey {
		DocumentKey {
			project: ProjectId::new("prj_a"),
			environment: Environment::parse("production").expect("valid"),
			name: DocumentName::parse("notes").expect("valid"),
		}
	}

	fn config() -> StripeConfig {
		StripeConfig {
			secret_key: Secret::new("sk_test_x"),
			meter_event: "canvist_active_document".to_owned(),
			conversion_meter_event: "canvist_conversion".to_owned(),
			api_base: "http://stripe.invalid".to_owned(),
			report_interval: Duration::from_secs(1),
			automatic_tax: false,
		}
	}

	#[test]
	fn document_identifiers_are_stable_per_document_and_month() {
		let october: Month = "2026-10".parse().expect("valid");
		let november: Month = "2026-11".parse().expect("valid");

		assert_eq!(
			document_identifier(&key(), october),
			document_identifier(&key(), october)
		);
		assert_ne!(
			document_identifier(&key(), october),
			document_identifier(&key(), november)
		);
		assert!(document_identifier(&key(), october).len() <= 100);
	}

	#[test]
	fn conversions_report_their_range_inside_their_month() {
		let config = config();
		let pending = PendingConversions {
			project: ProjectId::new("prj_a"),
			environment: Environment::parse("production").expect("valid"),
			month: "2026-10".parse().expect("valid"),
			customer: "cus_1".to_owned(),
			from: 5,
			to: 12,
		};
		let november_first = pending.month.next().start_unix_seconds();
		let event = conversions_event(&config, &pending, november_first + 60).expect("an event");

		assert_eq!(event.value, 7);
		assert_eq!(event.timestamp, november_first - 1);
		assert_eq!(event.identifier, conversions_identifier(&pending));

		let nothing = PendingConversions {
			from: 12,
			..pending
		};
		assert_eq!(conversions_event(&config, &nothing, november_first), None);
	}

	#[test]
	fn repeated_identifiers_count_as_sent() {
		assert!(is_duplicate(
			"An event already exists with identifier mad_0123456789abcdef."
		));
		assert!(!is_duplicate("No such customer: 'cus_missing'"));
	}
}
