//! The state every handler shares.

use std::sync::Arc;

use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use tokio::sync::watch;
use tokio_util::task::TaskTracker;
use tokio_util::task::task_tracker::TaskTrackerToken;

use crate::auth;
use crate::auth::Access;
use crate::auth::AuthError;
use crate::auth::Principal;
use crate::auth::TokenSigner;
use crate::config::Config;
use crate::convert::Converter;
use crate::error::ApiError;
use crate::metering::Admission;
use crate::metering::Meter;
use crate::metering::Month;
use crate::metering::stripe::StripeClient;
use crate::names::DocumentKey;
use crate::store::Store;
use crate::store::StoreError;
use crate::sync::Rooms;

/// Shared server state.
#[derive(Debug)]
pub struct App {
	/// The configuration.
	pub config: Config,
	/// The durable store.
	pub store: Store,
	/// Signs and verifies client tokens.
	pub signer: TokenSigner,
	/// Loaded documents.
	pub rooms: Rooms,
	/// Counts active documents and enforces caps.
	pub meter: Meter,
	/// Runs conversions.
	pub converter: Converter,
	/// Stripe, when billing is configured.
	pub stripe: Option<StripeClient>,
	shutdown: watch::Receiver<bool>,
	sockets: TaskTracker,
}

/// A handle to the shared state.
pub type SharedApp = Arc<App>;

impl App {
	/// Shared state for `config` over `store`.
	pub fn new(
		config: Config,
		store: Store,
		(rooms, meter): (Rooms, Meter),
		shutdown: watch::Receiver<bool>,
		sockets: TaskTracker,
	) -> Self {
		Self {
			signer: TokenSigner::new(config.token_secret.expose()),
			converter: Converter::new(config.convert.clone()),
			stripe: config.stripe.clone().map(StripeClient::new),
			config,
			store,
			rooms,
			meter,
			shutdown,
			sockets,
		}
	}

	/// A receiver that changes when the server begins shutting down.
	pub fn shutdown(&self) -> watch::Receiver<bool> {
		self.shutdown.clone()
	}

	/// Whether the server is shutting down.
	pub fn is_shutting_down(&self) -> bool {
		*self.shutdown.borrow()
	}

	/// A token that keeps shutdown waiting while a socket task runs.
	pub fn alive(&self) -> TaskTrackerToken {
		self.sockets.token()
	}

	/// Whether `key` may sync this month, counting it as active if it is
	/// new. A loaded room remembers its month, so later sessions skip the
	/// store.
	pub async fn admit(&self, key: &DocumentKey) -> Result<Admission, StoreError> {
		let month = Month::current();
		let room = self.rooms.get(key);

		if room.as_ref().is_some_and(|room| room.is_admitted(month)) {
			return Ok(Admission::Admitted);
		}

		let admission = self.meter.admit(key, month).await?;

		if let (Admission::Admitted, Some(room)) = (admission, room) {
			room.mark_admitted(month);
		}

		Ok(admission)
	}

	/// Resolve an API key or client token.
	pub async fn authenticate(&self, credential: &str) -> Result<Principal, ApiError> {
		if auth::is_api_key(credential) {
			let hash = auth::hash_api_key(credential);
			let (project, environment) = self
				.store
				.find_api_key(&hash)
				.await?
				.ok_or(AuthError::Invalid)?;
			return Ok(Principal {
				project,
				environment,
				access: Access::Key,
			});
		}

		let claims = self.signer.verify(credential, auth::unix_now())?;
		Ok(Principal {
			project: claims.prj.clone(),
			environment: claims.env.clone(),
			access: Access::Token(claims),
		})
	}

	/// The principal behind a request's `Authorization: Bearer` header.
	pub async fn authenticate_headers(&self, headers: &HeaderMap) -> Result<Principal, ApiError> {
		let credential = headers
			.get(AUTHORIZATION)
			.and_then(|value| value.to_str().ok())
			.and_then(auth::bearer)
			.ok_or(AuthError::Missing)?;
		self.authenticate(credential).await
	}

	/// The project environment behind an API key; client tokens are refused.
	pub async fn server_principal(&self, headers: &HeaderMap) -> Result<Principal, ApiError> {
		let principal = self.authenticate_headers(headers).await?;

		match principal.access {
			Access::Key => Ok(principal),
			Access::Token(_) => Err(AuthError::Forbidden.into()),
		}
	}

	/// Check the admin bearer token.
	pub fn authorize_admin(&self, headers: &HeaderMap) -> Result<(), ApiError> {
		let Some(expected) = &self.config.admin_token else {
			return Err(ApiError::not_found("admin routes are disabled"));
		};
		let given = headers
			.get(AUTHORIZATION)
			.and_then(|value| value.to_str().ok())
			.and_then(auth::bearer)
			.ok_or(AuthError::Missing)?;

		if auth::secrets_match(given, expected.expose()) {
			Ok(())
		} else {
			Err(AuthError::Invalid.into())
		}
	}
}
