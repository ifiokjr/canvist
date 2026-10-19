//! Reading and writing documents outside a WebSocket session.

use std::sync::Arc;

use axum::http::StatusCode;

use crate::app::App;
use crate::error::ApiError;
use crate::metering::Admission;
use crate::names::DocumentKey;
use crate::sync::validate::Rejected;

/// The editor name recorded for changes made through the REST API.
const SERVER: &str = "server";

/// `key`'s current state as one Yjs v1 update: live from its room when
/// loaded, otherwise from the store.
pub async fn current_state(app: &App, key: &DocumentKey) -> Result<Vec<u8>, ApiError> {
	if let Some(room) = app.rooms.get(key) {
		return Ok(room.encode_state());
	}

	let stored = app.store.load(key).await?;

	if stored.is_empty() {
		return Err(ApiError::not_found("no such document"));
	}

	let parts: Vec<&[u8]> = stored
		.snapshot
		.iter()
		.chain(&stored.updates)
		.map(Vec::as_slice)
		.collect();
	yrs::merge_updates_v1(&parts).map_err(|error| {
		tracing::error!(document = %key, %error, "stored document does not decode");
		ApiError::new(
			StatusCode::INTERNAL_SERVER_ERROR,
			"corrupt_document",
			"the stored document could not be read",
		)
	})
}

/// Apply a v1 `update` to `key` as the server, as a connected peer would.
/// The write counts the document as active this month; a new document on
/// a full free plan is refused.
pub async fn apply(app: &App, key: &DocumentKey, update: &[u8]) -> Result<bool, ApiError> {
	if app.admit(key).await? == Admission::Paused {
		return Err(ApiError::new(
			StatusCode::TOO_MANY_REQUESTS,
			"document_limit_reached",
			"the free plan's monthly active documents are used up; existing documents still \
			 accept writes",
		));
	}

	let (room, membership) = app.rooms.join(key).await?;
	let result = room.apply(
		None,
		&Arc::from(SERVER),
		update,
		crate::sync::Limits {
			max_update_bytes: app.config.sync.max_message_bytes,
			max_document_bytes: app.config.sync.max_document_bytes,
		},
	);
	room.leave(membership.peer);
	result.map_err(|error| rejected(&error))
}

fn rejected(rejected: &Rejected) -> ApiError {
	let status = match rejected {
		Rejected::TooLarge(_) | Rejected::DocumentFull => StatusCode::PAYLOAD_TOO_LARGE,
		Rejected::ReadOnly => StatusCode::FORBIDDEN,
		Rejected::Malformed | Rejected::ForeignRoot(_) | Rejected::NotABlock => {
			StatusCode::UNPROCESSABLE_ENTITY
		}
	};

	ApiError::new(status, "update_rejected", rejected.to_string())
}
