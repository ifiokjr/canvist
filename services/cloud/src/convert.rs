//! canvist Convert: documents between JSON, HTML, Markdown, and plain text.
//!
//! Conversions run on `canvist_headless` with TipTap's StarterKit, the same
//! engine the editor uses, so what the API writes is what the editor reads.
//! Each runs on the blocking pool, bounded by a semaphore, a size limit,
//! and a timeout, and counts toward the project's monthly usage.
//!
//! Convert is a route group of the same binary as Sync rather than its own
//! codebase: it shares the credentials, the store, and the usage counters,
//! and exporting a stored document needs Sync's documents. Railway still
//! runs it as its own service (`CANVIST_ROLE=convert`), so its CPU-bound
//! load scales apart from Sync's connections.

use std::sync::Arc;

use axum::http::StatusCode;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::canvist_extensions::StarterKitHandles;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as Json;
use tokio::sync::Semaphore;

use crate::config::ConvertConfig;
use crate::error::ApiError;

/// The yrs client id stored documents are read as. Reading never writes,
/// so the id never reaches a peer.
const READER_CLIENT: u64 = 1;

/// A conversion request: `POST /v1/convert`.
#[derive(Debug, Deserialize)]
pub struct ConvertRequest {
	/// The input's format: `json`, `html`, `markdown`, or `text`.
	pub from: String,
	/// The output's format.
	pub to: String,
	/// The input: a string, or for `json` a ProseMirror JSON object (or a
	/// string holding one).
	pub input: Json,
}

/// A conversion's result.
#[derive(Debug, Serialize)]
pub struct ConvertResponse {
	/// The output's format.
	pub format: &'static str,
	/// The output: a JSON object for `json`, a string otherwise.
	pub output: Json,
}

/// What a conversion reads.
enum Source {
	/// Text in a format.
	Text(Format, String),
	/// A stored document's Yjs state.
	Yjs(Vec<u8>),
}

/// Runs conversions under the instance's limits.
#[derive(Clone)]
pub struct Converter {
	engine: Engine<StarterKitHandles>,
	permits: Arc<Semaphore>,
	config: ConvertConfig,
}

impl std::fmt::Debug for Converter {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Converter")
			.field("config", &self.config)
			.finish_non_exhaustive()
	}
}

impl Converter {
	/// A converter with the StarterKit engine.
	pub fn new(config: ConvertConfig) -> Self {
		Self {
			engine: Engine::starter_kit(),
			permits: Arc::new(Semaphore::new(config.concurrency)),
			config,
		}
	}

	/// The conversion limits.
	pub fn config(&self) -> &ConvertConfig {
		&self.config
	}

	/// Convert `request`'s input.
	pub async fn convert(&self, request: ConvertRequest) -> Result<ConvertResponse, ApiError> {
		let from = parse_format(&request.from)?;
		let to = parse_format(&request.to)?;
		let input = input_text(from, request.input)?;
		self.run(Source::Text(from, input), to).await
	}

	/// Export a stored document's Yjs `state` as `to`.
	pub async fn export(&self, state: Vec<u8>, to: Format) -> Result<ConvertResponse, ApiError> {
		self.run(Source::Yjs(state), to).await
	}

	/// Run one conversion on the blocking pool. A conversion past the
	/// timeout is answered with `504` but still finishes in the background:
	/// blocking work cannot be cancelled, which is why the semaphore, not
	/// the timeout, bounds the load.
	async fn run(&self, source: Source, to: Format) -> Result<ConvertResponse, ApiError> {
		let permit = self
			.permits
			.clone()
			.acquire_owned()
			.await
			.map_err(|_| busy())?;
		let engine = self.engine.clone();
		let work = tokio::task::spawn_blocking(move || {
			let output = convert_blocking(&engine, source, to);
			drop(permit);
			output
		});

		let output = tokio::time::timeout(self.config.timeout, work)
			.await
			.map_err(|_| timed_out())?
			.map_err(|_| busy())?
			.map_err(|error| unconvertible(&error))?;

		Ok(ConvertResponse {
			format: to.name(),
			output,
		})
	}
}

/// Read `source` and write it as `to`.
fn convert_blocking(
	engine: &Engine<StarterKitHandles>,
	source: Source,
	to: Format,
) -> Result<Json, HeadlessError> {
	let doc = match source {
		Source::Text(from, input) => engine.parse(from, &input)?,
		Source::Yjs(state) => engine.from_yjs(&state, READER_CLIENT)?,
	};

	Ok(match to {
		Format::Json => doc.to_json(),
		_ => Json::String(doc.export(to)),
	})
}

fn parse_format(name: &str) -> Result<Format, ApiError> {
	name.parse().map_err(|_| {
		ApiError::bad_request(format!(
			"unknown format `{name}`; use json, html, markdown, or text"
		))
	})
}

/// The input as text: JSON objects are serialized, strings pass through.
fn input_text(from: Format, input: Json) -> Result<String, ApiError> {
	match (from, input) {
		(_, Json::String(text)) => Ok(text),
		(Format::Json, object @ Json::Object(_)) => Ok(object.to_string()),
		_ => {
			Err(ApiError::bad_request(
				"input must be a string, or an object for json",
			))
		}
	}
}

fn unconvertible(error: &HeadlessError) -> ApiError {
	ApiError::new(
		StatusCode::UNPROCESSABLE_ENTITY,
		"unconvertible",
		format!("{}: {error}", error.code()),
	)
}

fn timed_out() -> ApiError {
	ApiError::new(
		StatusCode::GATEWAY_TIMEOUT,
		"conversion_timeout",
		"the conversion took too long",
	)
}

fn busy() -> ApiError {
	ApiError::new(
		StatusCode::SERVICE_UNAVAILABLE,
		"converter_unavailable",
		"the converter is shutting down; retry",
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn text(from: Format, input: &str, to: Format) -> Json {
		convert_blocking(
			&Engine::starter_kit(),
			Source::Text(from, input.to_owned()),
			to,
		)
		.expect("converts")
	}

	#[test]
	fn markdown_becomes_html_and_back() {
		let html = text(Format::Markdown, "# Notes\n\nShip **it**.", Format::Html);
		assert_eq!(
			html,
			Json::String("<h1>Notes</h1><p>Ship <strong>it</strong>.</p>".to_owned())
		);
		let markdown = text(
			Format::Html,
			html.as_str().expect("a string"),
			Format::Markdown,
		);
		assert_eq!(markdown, Json::String("# Notes\n\nShip **it**.".to_owned()));
	}

	#[test]
	fn json_output_is_an_object() {
		let json = text(Format::Text, "one\ntwo", Format::Json);
		assert_eq!(json["type"], "doc");
		assert_eq!(json["content"].as_array().map(Vec::len), Some(2));
	}

	#[test]
	fn inputs_must_be_strings_or_json_objects() {
		assert!(input_text(Format::Markdown, Json::Bool(true)).is_err());
		assert!(input_text(Format::Markdown, serde_json::json!({ "a": 1 })).is_err());
		assert_eq!(
			input_text(Format::Json, serde_json::json!({ "type": "doc" })).ok(),
			Some(r#"{"type":"doc"}"#.to_owned())
		);
	}
}
