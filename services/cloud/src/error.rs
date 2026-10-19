//! Errors as JSON responses: `{ "error": { "code": "...", "message": "..." } }`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde_json::json;

use crate::auth::AuthError;
use crate::names::NameError;
use crate::store::StoreError;

/// An error a handler returns.
#[derive(Debug)]
pub struct ApiError {
	status: StatusCode,
	code: &'static str,
	message: String,
}

impl ApiError {
	/// An error with `status`, a stable machine-readable `code`, and a
	/// human-readable `message`.
	pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
		Self {
			status,
			code,
			message: message.into(),
		}
	}

	/// A malformed request.
	pub fn bad_request(message: impl Into<String>) -> Self {
		Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
	}

	/// Something that does not exist.
	pub fn not_found(message: impl Into<String>) -> Self {
		Self::new(StatusCode::NOT_FOUND, "not_found", message)
	}

	/// The status code.
	pub fn status(&self) -> StatusCode {
		self.status
	}
}

impl IntoResponse for ApiError {
	fn into_response(self) -> Response {
		let body = json!({ "error": { "code": self.code, "message": self.message } });
		(self.status, Json(body)).into_response()
	}
}

impl From<AuthError> for ApiError {
	fn from(error: AuthError) -> Self {
		let (status, code) = match error {
			AuthError::Missing => (StatusCode::UNAUTHORIZED, "missing_credentials"),
			AuthError::Invalid => (StatusCode::UNAUTHORIZED, "invalid_credentials"),
			AuthError::Expired => (StatusCode::UNAUTHORIZED, "token_expired"),
			AuthError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
		};

		Self::new(status, code, error.to_string())
	}
}

impl From<NameError> for ApiError {
	fn from(error: NameError) -> Self {
		Self::bad_request(error.to_string())
	}
}

impl From<StoreError> for ApiError {
	fn from(error: StoreError) -> Self {
		if matches!(error, StoreError::UnknownProject) {
			return Self::not_found("unknown project");
		}

		tracing::error!(%error, "store error");
		Self::new(
			StatusCode::SERVICE_UNAVAILABLE,
			"store_unavailable",
			"the document store is unavailable; retry shortly",
		)
	}
}
