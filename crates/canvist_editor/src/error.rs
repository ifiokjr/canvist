//! Editor errors.

use canvist_model::JsonError;
use canvist_model::SchemaError;
use canvist_model::StepError;

/// Why an editor operation failed.
///
/// Commands that do not apply to the current state return `Ok(None)`, not an
/// error. An error means a step was rejected by the model, which points at a
/// bug in the command or an invalid document, so callers should surface it.
/// The remaining variants are configuration mistakes caught while building
/// an [`Editor`](crate::Editor).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum EditorError {
	/// The model rejected a step. The transaction was rolled back.
	#[error(transparent)]
	Step(#[from] StepError),
	/// The extensions produced an invalid schema.
	#[error(transparent)]
	Schema(#[from] SchemaError),
	/// Layout could not answer a line or vertical motion question.
	#[error(transparent)]
	Navigation(#[from] crate::NavigationError),
	/// The initial content did not fit the schema.
	#[error(transparent)]
	Json(#[from] JsonError),
	/// An extension needs a block, mark, or embed type nobody registered.
	#[error("extension `{extension}` needs the {kind} type `{name}`")]
	MissingType {
		/// The extension asking for the type.
		extension: &'static str,
		/// `block`, `mark`, or `embed`.
		kind: &'static str,
		/// The missing type's name.
		name: &'static str,
	},
	/// Two extensions registered a command or query with the same name.
	#[error("two extensions register `{0}`")]
	DuplicateName(String),
	/// No extension registered a command or query with this name.
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
	/// A key binding could not be parsed.
	#[error("invalid key binding `{0}`")]
	InvalidKey(String),
	/// An input or paste rule pattern is not a valid regular expression, or
	/// an input rule pattern is not anchored to the caret with `$`.
	#[error("invalid rule pattern `{pattern}`: {reason}")]
	InvalidPattern {
		/// The pattern.
		pattern: String,
		/// What is wrong with it.
		reason: String,
	},
}
