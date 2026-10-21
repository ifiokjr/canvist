//! The one error type every exported method throws.

use canvist_headless::HeadlessError;
use canvist_layout::LayoutError;

/// A failure inside canvist, thrown as `CanvistError` in Swift and
/// `CanvistException` in Kotlin.
///
/// `code` is the stable name every binding shares (see
/// [`ErrorCode`](canvist_headless::ErrorCode)), such as `invalidArgs` or
/// `unknownName`, plus `render` when layout fails.
#[derive(Debug, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum CanvistError {
	/// Something went wrong; `code` says what kind of thing.
	#[error("{code}: {reason}")]
	Failed {
		/// The stable code, such as `invalidArgs`.
		code: String,
		/// What went wrong, for people. (Not `message`, which Kotlin
		/// exceptions already have.)
		reason: String,
	},
}

impl CanvistError {
	pub(crate) fn new(code: &str, reason: impl Into<String>) -> Self {
		Self::Failed {
			code: code.to_owned(),
			reason: reason.into(),
		}
	}

	/// Arguments that are not valid JSON, or not the JSON a call expects.
	pub(crate) fn invalid_args(name: &str, reason: impl std::fmt::Display) -> Self {
		Self::new(
			"invalidArgs",
			format!("invalid arguments for `{name}`: {reason}"),
		)
	}

	/// The stable code.
	#[must_use]
	pub fn code(&self) -> &str {
		match self {
			Self::Failed { code, .. } => code,
		}
	}
}

impl From<HeadlessError> for CanvistError {
	fn from(error: HeadlessError) -> Self {
		Self::new(error.code().as_str(), error.to_string())
	}
}

impl From<LayoutError> for CanvistError {
	fn from(error: LayoutError) -> Self {
		Self::new("render", error.to_string())
	}
}

pub(crate) type Result<T, E = CanvistError> = std::result::Result<T, E>;
