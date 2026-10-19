//! The HTTP API. See `docs/src/cloud/api.md` for the reference.

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::routing::put;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;

use crate::app::SharedApp;
use crate::auth;
use crate::auth::Claims;
use crate::auth::MAX_TOKEN_SECONDS;
use crate::auth::Principal;
use crate::auth::Scope;
use crate::error::ApiError;
use crate::names::DocumentKey;
use crate::names::DocumentName;
use crate::names::Environment;
use crate::names::random_token;
use crate::store::Webhook;
use crate::sync::hocuspocus;
use crate::sync::webhooks;
use crate::sync::ywebsocket;

/// The longest token subject (the customer's user id), in bytes.
const MAX_SUBJECT_BYTES: usize = 256;
/// The longest project name, in bytes.
const MAX_PROJECT_NAME_BYTES: usize = 128;
/// Random characters in a webhook secret: 160 bits.
const WEBHOOK_SECRET_LENGTH: usize = 32;

/// Every route this instance's role serves.
pub fn router(app: SharedApp) -> Router {
	let max_body = app.config.sync.max_message_bytes;
	let mut router = Router::new()
		.route("/", get(index))
		.route("/healthz", get(health))
		.route("/readyz", get(ready))
		.route("/v1/admin/projects", post(create_project))
		.route("/v1/admin/projects/{project}/keys", post(create_key))
		.route("/v1/admin/stats", get(stats));

	if app.config.role.serves_sync() {
		router = router.merge(sync_routes());
	}

	router
		.layer(tower_http::limit::RequestBodyLimitLayer::new(max_body))
		.layer(tower_http::trace::TraceLayer::new_for_http())
		.with_state(app)
}

/// The relay and the REST routes around it.
fn sync_routes() -> Router<SharedApp> {
	Router::new()
		.route("/v1/sync/{*document}", get(ywebsocket::connect))
		.route("/v1/hocuspocus", get(hocuspocus::connect))
		.route("/v1/hocuspocus/{*path}", get(hocuspocus::connect))
		.route("/v1/tokens", post(mint_token))
		.route("/v1/documents", get(list_documents))
		.route(
			"/v1/documents/{*name}",
			get(export_document)
				.post(apply_update)
				.delete(delete_document),
		)
		.route("/v1/webhook", put(set_webhook).delete(clear_webhook))
}

async fn index() -> Json<JsonValue> {
	Json(json!({
		"service": "canvist-cloud",
		"version": env!("CARGO_PKG_VERSION"),
		"docs": "https://github.com/ifiokjr/canvist/blob/main/docs/src/cloud/api.md",
	}))
}

async fn health() -> &'static str {
	"ok"
}

async fn ready(State(app): State<SharedApp>) -> Response {
	if app.is_shutting_down() {
		return (StatusCode::SERVICE_UNAVAILABLE, "shutting down").into_response();
	}

	match app.store.ping().await {
		Ok(()) => "ready".into_response(),
		Err(error) => {
			tracing::warn!(%error, "readiness check failed");
			(StatusCode::SERVICE_UNAVAILABLE, "store unavailable").into_response()
		}
	}
}

#[derive(Debug, Deserialize)]
struct TokenRequest {
	/// The customer's id for the user.
	subject: String,
	/// A document name, or a prefix ending in `*`.
	document: String,
	#[serde(default = "default_scope")]
	scope: Scope,
	/// Lifetime in seconds; at most a day.
	#[serde(default = "default_ttl")]
	ttl_seconds: u64,
}

fn default_scope() -> Scope {
	Scope::Write
}

fn default_ttl() -> u64 {
	60 * 60
}

#[derive(Debug, Serialize)]
struct TokenResponse {
	token: String,
	expires_at: u64,
}

/// `POST /v1/tokens`: mint a client token. API keys only.
async fn mint_token(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
	let principal = app.server_principal(&headers).await?;

	if request.subject.is_empty() || request.subject.len() > MAX_SUBJECT_BYTES {
		return Err(ApiError::bad_request("subject must be 1 to 256 bytes"));
	}

	if !(1..=MAX_TOKEN_SECONDS).contains(&request.ttl_seconds) {
		return Err(ApiError::bad_request(
			"ttl_seconds must be between 1 and 86400",
		));
	}

	let pattern = request
		.document
		.strip_suffix('*')
		.unwrap_or(&request.document);

	if !pattern.is_empty() {
		DocumentName::parse(pattern)?;
	}

	let now = auth::unix_now();
	let claims = Claims {
		prj: principal.project,
		env: principal.environment,
		sub: request.subject,
		doc: request.document,
		scope: request.scope,
		iat: now,
		exp: now + request.ttl_seconds,
	};

	Ok(Json(TokenResponse {
		token: app.signer.sign(&claims),
		expires_at: claims.exp,
	}))
}

async fn list_documents(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let documents = app
		.store
		.list(&principal.project, &principal.environment)
		.await?;
	Ok(Json(json!({ "documents": documents })))
}

fn document_key(principal: &Principal, name: &str) -> Result<DocumentKey, ApiError> {
	Ok(DocumentKey {
		project: principal.project.clone(),
		environment: principal.environment.clone(),
		name: DocumentName::parse(name)?,
	})
}

/// `GET /v1/documents/{name}`: the document as one Yjs v1 update.
async fn export_document(
	State(app): State<SharedApp>,
	Path(name): Path<String>,
	headers: HeaderMap,
) -> Result<Response, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let key = document_key(&principal, &name)?;
	let state = crate::documents::current_state(&app, &key).await?;
	Ok(([(CONTENT_TYPE, "application/octet-stream")], state).into_response())
}

/// `POST /v1/documents/{name}`: apply a Yjs v1 update from a server, as a
/// peer would. Connected peers receive it at once.
async fn apply_update(
	State(app): State<SharedApp>,
	Path(name): Path<String>,
	headers: HeaderMap,
	body: Bytes,
) -> Result<Json<JsonValue>, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let key = document_key(&principal, &name)?;
	let changed = crate::documents::apply(&app, &key, &body).await?;
	Ok(Json(json!({ "changed": changed })))
}

async fn delete_document(
	State(app): State<SharedApp>,
	Path(name): Path<String>,
	headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let key = document_key(&principal, &name)?;
	app.rooms.discard(&key);

	if app.store.delete(&key).await? {
		Ok(StatusCode::NO_CONTENT)
	} else {
		Err(ApiError::not_found("no such document"))
	}
}

#[derive(Debug, Deserialize)]
struct WebhookRequest {
	url: String,
}

/// `PUT /v1/webhook`: set the environment's webhook and return a fresh
/// signing secret.
async fn set_webhook(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<WebhookRequest>,
) -> Result<Json<JsonValue>, ApiError> {
	let principal = app.server_principal(&headers).await?;
	webhooks::validate_url(&request.url, app.config.sync.webhook_allow_private)
		.map_err(|error| ApiError::bad_request(error.to_string()))?;
	let webhook = Webhook {
		url: request.url,
		secret: format!("whsec_{}", random_token(WEBHOOK_SECRET_LENGTH)),
	};
	app.store
		.set_webhook(&principal.project, &principal.environment, Some(&webhook))
		.await?;
	Ok(Json(
		json!({ "url": webhook.url, "secret": webhook.secret }),
	))
}

async fn clear_webhook(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
	let principal = app.server_principal(&headers).await?;
	app.store
		.set_webhook(&principal.project, &principal.environment, None)
		.await?;
	Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct ProjectRequest {
	name: String,
	#[serde(default = "default_environments")]
	environments: Vec<Environment>,
}

fn default_environments() -> Vec<Environment> {
	["production", "staging", "development"]
		.into_iter()
		.filter_map(|name| Environment::parse(name).ok())
		.collect()
}

/// `POST /v1/admin/projects`: create a project and one API key per
/// environment.
async fn create_project(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<ProjectRequest>,
) -> Result<(StatusCode, Json<JsonValue>), ApiError> {
	app.authorize_admin(&headers)?;

	if request.name.is_empty() || request.name.len() > MAX_PROJECT_NAME_BYTES {
		return Err(ApiError::bad_request("name must be 1 to 128 bytes"));
	}

	let project = app.store.create_project(&request.name).await?;
	let mut keys = serde_json::Map::new();

	for environment in &request.environments {
		let key = auth::generate_api_key();
		app.store.add_api_key(&project, environment, &key).await?;
		keys.insert(environment.to_string(), key.secret.into());
	}

	Ok((
		StatusCode::CREATED,
		Json(json!({ "project": project, "name": request.name, "keys": keys })),
	))
}

#[derive(Debug, Deserialize)]
struct KeyRequest {
	environment: Environment,
}

async fn create_key(
	State(app): State<SharedApp>,
	Path(project): Path<String>,
	headers: HeaderMap,
	Json(request): Json<KeyRequest>,
) -> Result<(StatusCode, Json<JsonValue>), ApiError> {
	app.authorize_admin(&headers)?;
	let project = crate::names::ProjectId::new(project);
	let key = auth::generate_api_key();
	app.store
		.add_api_key(&project, &request.environment, &key)
		.await?;
	Ok((
		StatusCode::CREATED,
		Json(json!({ "project": project, "environment": request.environment, "key": key.secret })),
	))
}

async fn stats(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
	app.authorize_admin(&headers)?;
	Ok(Json(json!(app.rooms.stats())))
}
