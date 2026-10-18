//! Editor errors.

use canvist_model::SchemaError;
use canvist_model::StepError;

/// Why an editor operation failed.
///
/// Commands that do not apply to the current state return `Ok(None)`, not an
/// error. An error means a step was rejected by the model, which points at a
/// bug in the command or an invalid document, so callers should surface it.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EditorError {
	/// The model rejected a step. The transaction was rolled back.
	#[error(transparent)]
	Step(#[from] StepError),
	/// The extensions produced an invalid schema.
	#[error(transparent)]
	Schema(#[from] SchemaError),
}
