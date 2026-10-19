//! A running relay, its project, and JavaScript clients to point at it.

#![allow(dead_code, reason = "each test binary uses a different subset")]

pub mod deno;
pub mod stripe;

use std::net::SocketAddr;
use std::time::Duration;

use canvist_cloud::Config;
use canvist_cloud::Server;
use canvist_cloud::auth;
use canvist_cloud::auth::Claims;
use canvist_cloud::auth::Scope;
use canvist_cloud::auth::TokenSigner;
use canvist_cloud::names::Environment;
use canvist_cloud::names::ProjectId;
use canvist_cloud::store::Store;

/// The token secret every test server uses.
pub const SECRET: &str = "test-secret-test-secret-test-secret!";
/// The admin token every test server uses.
pub const ADMIN: &str = "test-admin-token";

/// A configuration tuned for tests: fast flushes and webhooks.
pub fn config() -> Config {
	let mut config = Config::local(SECRET);
	config.admin_token = Some(canvist_cloud::config::Secret::new(ADMIN));
	config.sync.flush_interval = Duration::from_millis(20);
	config.sync.webhook_debounce = Duration::from_millis(200);
	config.sync.webhook_max_wait = Duration::from_secs(5);
	config.sync.webhook_allow_private = true;
	config
}

/// The store tests run against: Postgres when `CANVIST_TEST_DATABASE_URL`
/// is set, memory otherwise.
pub async fn store() -> Store {
	match std::env::var("CANVIST_TEST_DATABASE_URL") {
		Ok(url) if !url.is_empty() => {
			Store::postgres(&url)
				.await
				.expect("the test database is reachable")
		}
		_ => Store::memory(),
	}
}

/// A database no other test shares: a fresh schema in
/// `CANVIST_TEST_DATABASE_URL`, or `None` to run in memory. Billing tests
/// need one, because a server with Stripe reports the whole database's usage
/// and nextest runs every test in its own process at once.
pub async fn isolated_database() -> Option<String> {
	let url = std::env::var("CANVIST_TEST_DATABASE_URL")
		.ok()
		.filter(|url| !url.is_empty())?;
	let schema = format!("test_{}", canvist_cloud::names::random_token(16));
	let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
		.await
		.expect("the test database is reachable");
	tokio::spawn(connection);
	client
		.batch_execute(&format!("CREATE SCHEMA {schema}"))
		.await
		.expect("creates a schema");
	let separator = if url.contains('?') { '&' } else { '?' };

	Some(format!(
		"{url}{separator}options=-c%20search_path%3D{schema}"
	))
}

/// The store over `database`, or a fresh in-memory one.
pub async fn store_at(database: Option<&str>) -> Store {
	match database {
		Some(url) => {
			Store::postgres(url)
				.await
				.expect("the test database is reachable")
		}
		None => Store::memory(),
	}
}

/// A server with one project.
pub struct Fixture {
	pub server: Server,
	pub store: Store,
	pub project: ProjectId,
	pub environment: Environment,
	pub api_key: String,
}

impl Fixture {
	/// Start a server over a fresh project in `store`.
	pub async fn start() -> Self {
		Self::start_with(config(), store().await).await
	}

	/// Start with `config` over `store`.
	pub async fn start_with(config: Config, store: Store) -> Self {
		let project = store.create_project("test").await.expect("project");
		let environment = Environment::parse("production").expect("valid");
		let key = auth::generate_api_key();
		store
			.add_api_key(&project, &environment, &key)
			.await
			.expect("key");
		let server = Server::start_with_store(config, store.clone())
			.await
			.expect("server starts");

		Self {
			server,
			store,
			project,
			environment,
			api_key: key.secret,
		}
	}

	/// Restart the server over the same store, as a deploy would.
	/// A Postgres store reconnects from scratch; a memory store is reused,
	/// standing in for the database.
	pub async fn restart(self, config: Config) -> Self {
		let store = match &self.store {
			Store::Memory(_) => self.store.clone(),
			Store::Postgres(_) => store().await,
		};
		self.restart_with(config, store).await
	}

	/// Restart the server over `store`, which holds the same data.
	pub async fn restart_with(self, config: Config, store: Store) -> Self {
		let report = self.server.shutdown().await;
		assert_eq!(report.persist_failures, 0);
		let server = Server::start_with_store(config, store.clone())
			.await
			.expect("server restarts");
		Self {
			server,
			store,
			..self
		}
	}

	pub fn addr(&self) -> SocketAddr {
		self.server.addr()
	}

	/// `http://…/<path>`.
	pub fn http(&self, path: &str) -> String {
		format!("http://{}{path}", self.addr())
	}

	/// The y-websocket server URL clients pass to `WebsocketProvider`.
	pub fn sync_url(&self) -> String {
		format!("ws://{}/v1/sync", self.addr())
	}

	/// The Hocuspocus URL clients pass to `HocuspocusProvider`.
	pub fn hocuspocus_url(&self) -> String {
		format!("ws://{}/v1/hocuspocus", self.addr())
	}

	/// A client token for `document` with `scope`, valid for an hour.
	pub fn token(&self, document: &str, scope: Scope) -> String {
		self.token_until(document, scope, auth::unix_now() + 3600)
	}

	/// A client token that expires at `exp`.
	pub fn token_until(&self, document: &str, scope: Scope, exp: u64) -> String {
		TokenSigner::new(SECRET).sign(&Claims {
			prj: self.project.clone(),
			env: self.environment.clone(),
			sub: format!("user-{document}"),
			doc: document.to_owned(),
			scope,
			iat: auth::unix_now(),
			exp,
		})
	}

	/// An HTTP client authenticated with the project's API key.
	pub fn api(&self) -> Api {
		Api {
			client: reqwest::Client::new(),
			base: self.http(""),
			key: self.api_key.clone(),
		}
	}
}

/// REST calls with the project's API key.
pub struct Api {
	pub client: reqwest::Client,
	pub base: String,
	pub key: String,
}

impl Api {
	pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
		self.client
			.get(format!("{}{path}", self.base))
			.bearer_auth(&self.key)
	}

	pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
		self.client
			.post(format!("{}{path}", self.base))
			.bearer_auth(&self.key)
	}

	pub fn put(&self, path: &str) -> reqwest::RequestBuilder {
		self.client
			.put(format!("{}{path}", self.base))
			.bearer_auth(&self.key)
	}

	pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
		self.client
			.delete(format!("{}{path}", self.base))
			.bearer_auth(&self.key)
	}
}

/// Poll `check` every 10 ms for up to five seconds.
pub async fn eventually(mut check: impl FnMut() -> bool) -> bool {
	for _ in 0..500 {
		if check() {
			return true;
		}

		tokio::time::sleep(Duration::from_millis(10)).await;
	}

	false
}
