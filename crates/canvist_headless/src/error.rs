//! Errors, with codes every binding shares.

use canvist_editor::EditorError;
use canvist_model::JsonError;
use canvist_model::StepError;

/// Why a headless operation failed.
///
/// Every variant has a stable [`ErrorCode`], which the TypeScript and Dart
/// bindings and the conformance fixtures use, so a failure means the same
/// thing in every language.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HeadlessError {
	/// The extensions do not form a valid editor, e.g. two register the
	/// same command.
	#[error("invalid configuration: {0}")]
	Config(EditorError),
	/// The input could not be read in the format it was given as.
	#[error("could not parse {format}: {reason}")]
	Parse {
		/// The format name.
		format: &'static str,
		/// What went wrong.
		reason: String,
	},
	/// The content does not fit the schema.
	#[error("content does not fit the schema: {0}")]
	Schema(String),
	/// No registered extension provides a command or query by this name.
	#[error("no command or query is named `{0}`")]
	UnknownName(String),
	/// Arguments did not match a command's or query's declaration.
	#[error("invalid arguments for `{name}`: {reason}")]
	InvalidArgs {
		/// The command or query.
		name: String,
		/// What is wrong.
		reason: String,
	},
	/// A position does not address a text block, or its offset does not fit
	/// that block's text.
	#[error("invalid position: {0}")]
	InvalidPosition(String),
	/// A key binding such as `Mod-b` could not be parsed.
	#[error("invalid key `{0}`")]
	InvalidKey(String),
	/// A command failed in the editing engine. This points at a defect in
	/// the command, not at the caller.
	#[error(transparent)]
	Editor(EditorError),
	/// A snapshot could not be read.
	#[error("invalid snapshot: {0}")]
	Snapshot(String),
	/// The document is not shared, so it has no Yjs state.
	#[error("the document is not shared; call `share` first")]
	NotShared,
	/// The document is already shared.
	#[error("the document is already shared")]
	AlreadyShared,
	/// A Yjs update or sync message failed.
	#[cfg(feature = "collab")]
	#[error(transparent)]
	Collab(#[from] canvist_collab::CollabError),
	/// Layout failed while rendering.
	#[cfg(feature = "ssr")]
	#[error(transparent)]
	Layout(#[from] canvist_layout::LayoutError),
}

/// A stable, language-neutral name for each kind of [`HeadlessError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
	/// [`HeadlessError::Config`].
	Config,
	/// [`HeadlessError::Parse`].
	Parse,
	/// [`HeadlessError::Schema`].
	Schema,
	/// [`HeadlessError::UnknownName`].
	UnknownName,
	/// [`HeadlessError::InvalidArgs`].
	InvalidArgs,
	/// [`HeadlessError::InvalidPosition`].
	InvalidPosition,
	/// [`HeadlessError::InvalidKey`].
	InvalidKey,
	/// [`HeadlessError::Editor`].
	Editor,
	/// [`HeadlessError::Snapshot`].
	Snapshot,
	/// [`HeadlessError::NotShared`].
	NotShared,
	/// [`HeadlessError::AlreadyShared`].
	AlreadyShared,
	/// A collaboration failure.
	Collab,
	/// A rendering failure.
	Render,
}

impl ErrorCode {
	/// The code as the bindings spell it, e.g. `unknownName`.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Config => "config",
			Self::Parse => "parse",
			Self::Schema => "schema",
			Self::UnknownName => "unknownName",
			Self::InvalidArgs => "invalidArgs",
			Self::InvalidPosition => "invalidPosition",
			Self::InvalidKey => "invalidKey",
			Self::Editor => "editor",
			Self::Snapshot => "snapshot",
			Self::NotShared => "notShared",
			Self::AlreadyShared => "alreadyShared",
			Self::Collab => "collab",
			Self::Render => "render",
		}
	}
}

impl std::fmt::Display for ErrorCode {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str(self.as_str())
	}
}

impl HeadlessError {
	/// The error's stable code.
	#[must_use]
	pub fn code(&self) -> ErrorCode {
		match self {
			Self::Config(_) => ErrorCode::Config,
			Self::Parse { .. } => ErrorCode::Parse,
			Self::Schema(_) => ErrorCode::Schema,
			Self::UnknownName(_) => ErrorCode::UnknownName,
			Self::InvalidArgs { .. } => ErrorCode::InvalidArgs,
			Self::InvalidPosition(_) => ErrorCode::InvalidPosition,
			Self::InvalidKey(_) => ErrorCode::InvalidKey,
			Self::Editor(_) => ErrorCode::Editor,
			Self::Snapshot(_) => ErrorCode::Snapshot,
			Self::NotShared => ErrorCode::NotShared,
			Self::AlreadyShared => ErrorCode::AlreadyShared,
			#[cfg(feature = "collab")]
			Self::Collab(_) => ErrorCode::Collab,
			#[cfg(feature = "ssr")]
			Self::Layout(_) => ErrorCode::Render,
		}
	}

	pub(crate) fn parse(format: &'static str, reason: impl std::fmt::Display) -> Self {
		Self::Parse {
			format,
			reason: reason.to_string(),
		}
	}
}

impl From<EditorError> for HeadlessError {
	fn from(error: EditorError) -> Self {
		match error {
			EditorError::UnknownName(name) => Self::UnknownName(name),
			EditorError::InvalidArgs { name, reason } => Self::InvalidArgs { name, reason },
			EditorError::InvalidKey(key) => Self::InvalidKey(key),
			EditorError::Json(error) => error.into(),
			error => Self::Editor(error),
		}
	}
}

impl From<JsonError> for HeadlessError {
	fn from(error: JsonError) -> Self {
		match error {
			JsonError::Shape { .. } => Self::parse("json", error),
			JsonError::UnknownType { .. } | JsonError::Step(_) => Self::Schema(error.to_string()),
		}
	}
}

impl From<StepError> for HeadlessError {
	fn from(error: StepError) -> Self {
		Self::Schema(error.to_string())
	}
}
