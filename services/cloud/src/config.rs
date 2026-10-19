//! Configuration, read once from the environment at startup.
//!
//! Every setting has a production-safe default except the token secret,
//! which must be set. See `docs/src/cloud/self-hosting.md` for the full
//! list.

use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

/// Which route groups this process serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
	/// Sync and Convert in one process: the self-hosting default.
	All,
	/// Only the y-sync relay and the API it needs.
	Sync,
	/// Only the stateless conversion endpoints.
	Convert,
}

impl Role {
	/// Whether this role serves the sync relay.
	pub fn serves_sync(self) -> bool {
		matches!(self, Self::All | Self::Sync)
	}

	/// Whether this role serves conversions.
	pub fn serves_convert(self) -> bool {
		matches!(self, Self::All | Self::Convert)
	}
}

impl FromStr for Role {
	type Err = ConfigError;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		match value {
			"all" => Ok(Self::All),
			"sync" => Ok(Self::Sync),
			"convert" => Ok(Self::Convert),
			_ => {
				Err(ConfigError::invalid(
					"CANVIST_ROLE",
					"expected all, sync, or convert",
				))
			}
		}
	}
}

/// A secret that never prints.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
	/// Wrap `value`.
	pub fn new(value: impl Into<String>) -> Self {
		Self(value.into())
	}

	/// The secret's value.
	pub fn expose(&self) -> &str {
		&self.0
	}
}

impl std::fmt::Debug for Secret {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("Secret(..)")
	}
}

const MEBIBYTE: usize = 1024 * 1024;
const FLUSH_MILLIS: u64 = 250;
const COMPACT_AFTER_UPDATES: usize = 200;
const MAX_DOCUMENT_MEBIBYTES: usize = 16;
const WEBHOOK_DEBOUNCE_SECONDS: u64 = 2;
const WEBHOOK_MAX_WAIT_SECONDS: u64 = 30;
const PAUSE_RECHECK_SECONDS: u64 = 30;
const MAX_CONVERT_MEBIBYTES: usize = 8;
const CONVERT_TIMEOUT_SECONDS: u64 = 10;
/// The port when `PORT` is unset; Railway always sets it.
const DEFAULT_PORT: u16 = 8080;
/// The shortest token secret: 256 bits, HS256's key size.
const MIN_TOKEN_SECRET_BYTES: usize = 32;

/// Settings for the sync relay.
#[derive(Clone, Debug)]
pub struct SyncConfig {
	/// How long a document with no connections stays in memory.
	pub idle_eviction: Duration,
	/// How often pending updates are written to the store.
	pub flush_interval: Duration,
	/// Stored update rows after which a document is compacted into a
	/// snapshot.
	pub compact_after: usize,
	/// The largest WebSocket message accepted, in bytes.
	pub max_message_bytes: usize,
	/// The largest document accepted, as an encoded Yjs state, in bytes.
	pub max_document_bytes: usize,
	/// Quiet time after a change before its webhook fires.
	pub webhook_debounce: Duration,
	/// The longest a webhook waits while changes keep arriving.
	pub webhook_max_wait: Duration,
	/// Allow plain-HTTP webhooks and private hosts, for tests and
	/// self-hosting inside a private network.
	pub webhook_allow_private: bool,
	/// How often a paused session (a new document on a full free plan)
	/// checks whether it may sync now.
	pub pause_recheck: Duration,
}

impl Default for SyncConfig {
	fn default() -> Self {
		Self {
			idle_eviction: Duration::from_mins(1),
			flush_interval: Duration::from_millis(FLUSH_MILLIS),
			compact_after: COMPACT_AFTER_UPDATES,
			max_message_bytes: MEBIBYTE,
			max_document_bytes: MAX_DOCUMENT_MEBIBYTES * MEBIBYTE,
			webhook_debounce: Duration::from_secs(WEBHOOK_DEBOUNCE_SECONDS),
			webhook_max_wait: Duration::from_secs(WEBHOOK_MAX_WAIT_SECONDS),
			webhook_allow_private: false,
			pause_recheck: Duration::from_secs(PAUSE_RECHECK_SECONDS),
		}
	}
}

/// Settings for conversions.
#[derive(Clone, Debug)]
pub struct ConvertConfig {
	/// The largest request body accepted, in bytes.
	pub max_input_bytes: usize,
	/// The longest one conversion may run before the request gives up.
	pub timeout: Duration,
	/// Conversions running at once on this instance.
	pub concurrency: usize,
}

impl Default for ConvertConfig {
	fn default() -> Self {
		Self {
			max_input_bytes: MAX_CONVERT_MEBIBYTES * MEBIBYTE,
			timeout: Duration::from_secs(CONVERT_TIMEOUT_SECONDS),
			concurrency: std::thread::available_parallelism().map_or(2, |count| count.get() * 2),
		}
	}
}

/// Stripe Billing Meters, test mode only.
#[derive(Clone, Debug)]
pub struct StripeConfig {
	/// A `sk_test_` or `rk_test_` key. Live keys are refused at startup.
	pub secret_key: Secret,
	/// The meter's event name.
	pub meter_event: String,
	/// The Stripe API base URL, overridable for tests.
	pub api_base: String,
}

/// Everything the server needs.
#[derive(Clone, Debug)]
pub struct Config {
	/// Where to listen.
	pub bind: SocketAddr,
	/// Which route groups to serve.
	pub role: Role,
	/// Postgres connection string; `None` keeps everything in memory.
	pub database_url: Option<Secret>,
	/// The HMAC key that signs client tokens. At least 32 bytes.
	pub token_secret: Secret,
	/// The bearer token for `/v1/admin`; admin routes are off without it.
	pub admin_token: Option<Secret>,
	/// Sync relay settings.
	pub sync: SyncConfig,
	/// Conversion settings.
	pub convert: ConvertConfig,
	/// Stripe metering, off when `None`.
	pub stripe: Option<StripeConfig>,
	/// The signing secret of the Stripe webhook endpoint; the endpoint is off
	/// without it.
	pub stripe_webhook_secret: Option<Secret>,
}

impl Config {
	/// A configuration for tests and embedding: in memory, every role, and
	/// an ephemeral port.
	pub fn local(token_secret: impl Into<String>) -> Self {
		Self {
			bind: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
			role: Role::All,
			database_url: None,
			token_secret: Secret::new(token_secret),
			admin_token: None,
			sync: SyncConfig::default(),
			convert: ConvertConfig::default(),
			stripe: None,
			stripe_webhook_secret: None,
		}
	}

	/// Read the configuration from environment variables.
	pub fn from_env() -> Result<Self, ConfigError> {
		Self::from_lookup(|name| std::env::var(name).ok())
	}

	/// Read the configuration through `lookup`, which returns a variable's
	/// value.
	pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
		let env = Env(&lookup);
		let token_secret = env.require("CANVIST_TOKEN_SECRET")?;

		if token_secret.len() < MIN_TOKEN_SECRET_BYTES {
			return Err(ConfigError::invalid(
				"CANVIST_TOKEN_SECRET",
				"must be at least 32 bytes",
			));
		}

		Ok(Self {
			bind: SocketAddr::new(
				env.parse("HOST", IpAddr::V6(Ipv6Addr::UNSPECIFIED))?,
				env.parse("PORT", DEFAULT_PORT)?,
			),
			role: env.parse("CANVIST_ROLE", Role::All)?,
			database_url: env.get("DATABASE_URL").map(Secret::new),
			token_secret: Secret::new(token_secret),
			admin_token: env.get("CANVIST_ADMIN_TOKEN").map(Secret::new),
			sync: sync_config(&env)?,
			convert: convert_config(&env)?,
			stripe: stripe_config(&env)?,
			stripe_webhook_secret: env.get("STRIPE_WEBHOOK_SECRET").map(Secret::new),
		})
	}
}

fn sync_config(env: &Env<'_>) -> Result<SyncConfig, ConfigError> {
	let defaults = SyncConfig::default();

	Ok(SyncConfig {
		idle_eviction: env.seconds("CANVIST_IDLE_EVICTION_SECONDS", defaults.idle_eviction)?,
		flush_interval: env.millis("CANVIST_FLUSH_INTERVAL_MS", defaults.flush_interval)?,
		compact_after: env.parse("CANVIST_COMPACT_AFTER_UPDATES", defaults.compact_after)?,
		max_message_bytes: env.parse("CANVIST_MAX_MESSAGE_BYTES", defaults.max_message_bytes)?,
		max_document_bytes: env.parse("CANVIST_MAX_DOCUMENT_BYTES", defaults.max_document_bytes)?,
		webhook_debounce: env.millis("CANVIST_WEBHOOK_DEBOUNCE_MS", defaults.webhook_debounce)?,
		webhook_max_wait: env.millis("CANVIST_WEBHOOK_MAX_WAIT_MS", defaults.webhook_max_wait)?,
		webhook_allow_private: env.parse(
			"CANVIST_WEBHOOK_ALLOW_PRIVATE",
			defaults.webhook_allow_private,
		)?,
		pause_recheck: env.seconds("CANVIST_PAUSE_RECHECK_SECONDS", defaults.pause_recheck)?,
	})
}

fn convert_config(env: &Env<'_>) -> Result<ConvertConfig, ConfigError> {
	let defaults = ConvertConfig::default();

	Ok(ConvertConfig {
		max_input_bytes: env.parse("CANVIST_CONVERT_MAX_BYTES", defaults.max_input_bytes)?,
		timeout: env.millis("CANVIST_CONVERT_TIMEOUT_MS", defaults.timeout)?,
		concurrency: env.parse("CANVIST_CONVERT_CONCURRENCY", defaults.concurrency)?,
	})
}

fn stripe_config(env: &Env<'_>) -> Result<Option<StripeConfig>, ConfigError> {
	let Some(secret_key) = env.get("STRIPE_SECRET_KEY") else {
		return Ok(None);
	};

	if !(secret_key.starts_with("sk_test_") || secret_key.starts_with("rk_test_")) {
		return Err(ConfigError::invalid(
			"STRIPE_SECRET_KEY",
			"only test-mode keys (sk_test_ or rk_test_) are accepted",
		));
	}

	Ok(Some(StripeConfig {
		secret_key: Secret::new(secret_key),
		meter_event: env
			.get("STRIPE_METER_EVENT")
			.unwrap_or_else(|| "canvist_active_document".to_owned()),
		api_base: env
			.get("STRIPE_API_BASE")
			.unwrap_or_else(|| "https://api.stripe.com".to_owned()),
	}))
}

/// Typed access to variables, treating empty values as unset.
struct Env<'a>(&'a dyn Fn(&str) -> Option<String>);

impl Env<'_> {
	fn get(&self, name: &str) -> Option<String> {
		(self.0)(name).filter(|value| !value.is_empty())
	}

	fn require(&self, name: &str) -> Result<String, ConfigError> {
		self.get(name)
			.ok_or_else(|| ConfigError::invalid(name, "is required"))
	}

	fn parse<T: FromStr>(&self, name: &str, default: T) -> Result<T, ConfigError> {
		self.get(name).map_or(Ok(default), |value| {
			value
				.parse()
				.map_err(|_| ConfigError::invalid(name, "could not be parsed"))
		})
	}

	fn seconds(&self, name: &str, default: Duration) -> Result<Duration, ConfigError> {
		Ok(Duration::from_secs(self.parse(name, default.as_secs())?))
	}

	fn millis(&self, name: &str, default: Duration) -> Result<Duration, ConfigError> {
		let millis = u64::try_from(default.as_millis()).unwrap_or(u64::MAX);
		Ok(Duration::from_millis(self.parse(name, millis)?))
	}
}

/// A variable that is missing or malformed.
#[derive(Debug, thiserror::Error)]
#[error("{name} {problem}")]
pub struct ConfigError {
	name: String,
	problem: &'static str,
}

impl ConfigError {
	fn invalid(name: &str, problem: &'static str) -> Self {
		Self {
			name: name.to_owned(),
			problem,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

	fn config(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
		let vars: HashMap<String, String> = pairs
			.iter()
			.map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
			.collect();
		Config::from_lookup(|name| vars.get(name).cloned())
	}

	const SECRET: (&str, &str) = ("CANVIST_TOKEN_SECRET", "0123456789abcdef0123456789abcdef");

	#[test]
	fn the_token_secret_is_required_and_long() {
		assert!(config(&[]).is_err());
		assert!(config(&[("CANVIST_TOKEN_SECRET", "short")]).is_err());
		assert!(config(&[SECRET]).is_ok());
	}

	#[test]
	fn live_stripe_keys_are_refused() {
		assert!(config(&[SECRET, ("STRIPE_SECRET_KEY", "sk_live_abc")]).is_err());
		assert!(config(&[SECRET, ("STRIPE_SECRET_KEY", "rk_live_abc")]).is_err());
		let test = config(&[SECRET, ("STRIPE_SECRET_KEY", "sk_test_abc")]).expect("valid");
		assert_eq!(
			test.stripe.map(|stripe| stripe.meter_event),
			Some("canvist_active_document".to_owned())
		);
	}

	#[test]
	fn railway_port_is_honoured() {
		let config = config(&[SECRET, ("PORT", "4321")]).expect("valid");
		assert_eq!(config.bind.port(), 4321);
		assert_eq!(config.role, Role::All);
	}
}
