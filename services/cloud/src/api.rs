//! The HTTP API. See `docs/src/cloud/api.md` for the reference.

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::extract::Path;
use axum::extract::Query;
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

use crate::app::App;
use crate::app::SharedApp;
use crate::auth;
use crate::auth::Claims;
use crate::auth::MAX_TOKEN_SECONDS;
use crate::auth::Principal;
use crate::auth::Scope;
use crate::convert::ConvertRequest;
use crate::convert::ConvertResponse;
use crate::error::ApiError;
use crate::metering::Month;
use crate::metering::Plan;
use crate::metering::Standing;
use crate::metering::Terms;
use crate::metering::billing;
use crate::metering::billing::BillingError;
use crate::metering::stripe::StripeClient;
use crate::metering::stripe_webhook;
use crate::metering::stripe_webhook::Action;
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
		.route("/v1/admin/stats", get(stats))
		.route("/v1/admin/projects/{project}/terms", put(set_terms))
		.route("/v1/usage", get(usage))
		.route("/v1/stripe/webhook", post(stripe_webhook))
		.route("/v1/billing/checkout", post(checkout))
		.route("/v1/billing/portal", post(billing_portal))
		.route("/v1/billing/plan", post(change_plan));

	if app.config.role.serves_sync() {
		router = router.merge(sync_routes().layer(body_limit(max_body)));
	}

	if app.config.role.serves_convert() {
		router =
			router.merge(convert_routes().layer(body_limit(app.config.convert.max_input_bytes)));
	}

	router
		.layer(body_limit(max_body))
		.layer(tower_http::trace::TraceLayer::new_for_http())
		.with_state(app)
}

/// Bodies larger than `bytes` are refused with `413`. A route takes its
/// innermost limit, so Convert's larger one wins for its routes.
fn body_limit(bytes: usize) -> DefaultBodyLimit {
	DefaultBodyLimit::max(bytes)
}

/// Conversions.
fn convert_routes() -> Router<SharedApp> {
	Router::new().route("/v1/convert", post(convert))
}

/// `POST /v1/convert`: convert a document between formats. API keys only;
/// each conversion counts toward the month's usage.
async fn convert(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<ConvertRequest>,
) -> Result<Json<ConvertResponse>, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let response = app.converter.convert(request).await?;
	count_conversion(&app, &principal).await;
	Ok(Json(response))
}

/// Count a conversion. A failure to count is logged, not passed on: the
/// customer already has their result.
async fn count_conversion(app: &App, principal: &Principal) {
	let counted = app
		.store
		.add_conversions(
			&principal.project,
			&principal.environment,
			Month::current(),
			1,
		)
		.await;

	if let Err(error) = counted {
		tracing::warn!(project = %principal.project, %error, "could not count a conversion");
	}
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

#[derive(Debug, Deserialize)]
struct ExportQuery {
	format: Option<String>,
}

/// `GET /v1/documents/{name}?format=…`: the document as one Yjs v1 update
/// (the default, `yjs`), or converted to `json`, `html`, `markdown`, or
/// `text`, which counts as a conversion.
async fn export_document(
	State(app): State<SharedApp>,
	Path(name): Path<String>,
	Query(query): Query<ExportQuery>,
	headers: HeaderMap,
) -> Result<Response, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let key = document_key(&principal, &name)?;
	let state = crate::documents::current_state(&app, &key).await?;
	let format = match query.format.as_deref() {
		None | Some("yjs") => {
			return Ok(([(CONTENT_TYPE, "application/octet-stream")], state).into_response());
		}
		Some(name) => {
			name.parse().map_err(|_| {
				ApiError::bad_request(format!(
					"unknown format `{name}`; use yjs, json, html, markdown, or text"
				))
			})?
		}
	};
	let converted = app.converter.export(state, format).await?;
	count_conversion(&app, &principal).await;

	Ok(Json(converted).into_response())
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

#[derive(Debug, Deserialize)]
struct UsageQuery {
	month: Option<String>,
}

/// `GET /v1/usage?month=YYYY-MM`: the project's active documents and
/// conversions this month (or `month`), against its plan.
async fn usage(
	State(app): State<SharedApp>,
	Query(query): Query<UsageQuery>,
	headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let month = match query.month {
		Some(month) => {
			month
				.parse::<Month>()
				.map_err(|error| ApiError::bad_request(error.to_string()))?
		}
		None => Month::current(),
	};
	let terms = app.store.terms(&principal.project).await?;
	let environments = app.store.usage(&principal.project, month).await?;
	let active: u64 = environments.iter().map(|row| row.active_documents).sum();

	Ok(Json(json!({
		"project": principal.project,
		"month": month.to_string(),
		"plan": terms.plan,
		"included_documents": terms.included_documents,
		"active_documents": active,
		"standing": Standing::of(&terms, active),
		// `past_due` means a payment failed and Stripe is retrying it: the
		// plan still applies, but the app should ask for a new card.
		"subscription_status": terms.subscription.map(|subscription| subscription.status),
		"environments": environments,
	})))
}

/// `POST /v1/stripe/webhook`: subscription, checkout, and invoice events.
/// Each names a subscription, which is retrieved from Stripe and applied as
/// it is now, so out-of-order and repeated events are harmless. A completed
/// annual checkout creates its subscription first, idempotently. Off (`404`)
/// without `STRIPE_WEBHOOK_SECRET` and `STRIPE_SECRET_KEY`. Events for other
/// products or unknown projects are acknowledged and ignored, so Stripe
/// does not retry them; a Stripe or store failure answers `503` so it does.
async fn stripe_webhook(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	body: Bytes,
) -> Result<Json<JsonValue>, ApiError> {
	let (Some(secret), Some(stripe)) = (&app.config.stripe_webhook_secret, &app.stripe) else {
		return Err(ApiError::not_found("Stripe webhooks are not configured"));
	};
	let signature = headers
		.get("stripe-signature")
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default();
	stripe_webhook::verify(secret.expose(), signature, &body, auth::unix_now())
		.map_err(|error| ApiError::bad_request(error.to_string()))?;
	let event: JsonValue = serde_json::from_slice(&body)
		.map_err(|_| ApiError::bad_request(stripe_webhook::WebhookError::Payload.to_string()))?;
	let applied = match stripe_webhook::action(&event) {
		None => return Ok(Json(json!({ "handled": false }))),
		Some(Action::CompleteAnnualCheckout(session)) => {
			billing::complete_annual_checkout(stripe, &app.store, session).await?
		}
		Some(Action::Sync(subscription)) => {
			if event.get("type").and_then(JsonValue::as_str) == Some("invoice.payment_failed") {
				tracing::warn!(
					subscription,
					"a subscription payment failed; Stripe is retrying it"
				);
			}

			billing::sync_subscription(stripe, &app.store, subscription).await?
		}
	};

	Ok(Json(json!({ "handled": applied.is_some() })))
}

/// The configured Stripe client, or `404` where billing is off.
fn stripe(app: &App) -> Result<&StripeClient, ApiError> {
	app.stripe
		.as_ref()
		.ok_or_else(|| BillingError::NotConfigured.into())
}

#[derive(Debug, Serialize)]
struct UrlResponse {
	url: String,
}

/// `POST /v1/billing/checkout`: a Checkout Session for a Builder or Scale
/// subscription, billed monthly or (`"interval": "year"`) yearly. API keys
/// only.
async fn checkout(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<billing::CheckoutRequest>,
) -> Result<Json<UrlResponse>, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let url = billing::checkout(stripe(&app)?, &app.store, &principal.project, &request).await?;
	Ok(Json(UrlResponse { url }))
}

#[derive(Debug, Deserialize)]
struct PortalRequest {
	return_url: String,
}

/// `POST /v1/billing/portal`: a Customer Portal session for the project's
/// payment methods, invoices, billing details, and cancellation. API keys
/// only.
async fn billing_portal(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<PortalRequest>,
) -> Result<Json<UrlResponse>, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let url = billing::portal(
		stripe(&app)?,
		&app.store,
		&principal.project,
		&request.return_url,
	)
	.await?;
	Ok(Json(UrlResponse { url }))
}

#[derive(Debug, Deserialize)]
struct PlanRequest {
	plan: Plan,
}

/// `POST /v1/billing/plan`: switch a paid subscription between Builder and
/// Scale. API keys only.
async fn change_plan(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<PlanRequest>,
) -> Result<Json<JsonValue>, ApiError> {
	let principal = app.server_principal(&headers).await?;
	let terms =
		billing::change_plan(stripe(&app)?, &app.store, &principal.project, request.plan).await?;
	Ok(Json(
		json!({ "project": principal.project, "terms": terms }),
	))
}

#[derive(Debug, Deserialize)]
struct TermsRequest {
	plan: Plan,
	included_documents: Option<u64>,
	stripe_customer: Option<String>,
}

/// `PUT /v1/admin/projects/{project}/terms`: change a project's plan.
async fn set_terms(
	State(app): State<SharedApp>,
	Path(project): Path<String>,
	headers: HeaderMap,
	Json(request): Json<TermsRequest>,
) -> Result<Json<JsonValue>, ApiError> {
	app.authorize_admin(&headers)?;
	let project = crate::names::ProjectId::new(project);
	let current = app.store.terms(&project).await?;
	let terms = Terms {
		plan: request.plan,
		included_documents: request
			.included_documents
			.or_else(|| request.plan.included_documents()),
		stripe_customer: request.stripe_customer,
		subscription: current.subscription,
	};
	app.store.set_terms(&project, &terms).await?;

	Ok(Json(json!({ "project": project, "terms": terms })))
}

async fn stats(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
	app.authorize_admin(&headers)?;
	Ok(Json(json!(app.rooms.stats())))
}
