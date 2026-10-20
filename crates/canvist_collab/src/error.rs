//! Collaboration errors.

use canvist_model::BlockId;
use canvist_model::StepError;

/// Why a collaboration operation failed.
///
/// Errors that report a mismatch between the model and the shared document
/// ([`CollabError::NotShared`], [`CollabError::Offset`]) mean the two have
/// drifted apart, usually because the [`Document`](canvist_model::Document)
/// was changed without telling the [`Binding`](crate::Binding). Rebuild the
/// document with [`Binding::document`](crate::Binding::document) to recover.
#[derive(Debug, thiserror::Error)]
pub enum CollabError {
	/// A translated step was rejected by the model.
	#[error(transparent)]
	Step(#[from] StepError),
	/// The block has no counterpart in the shared document.
	#[error("block {0} is not in the shared document")]
	NotShared(BlockId),
	/// A text offset fell outside, or inside a character of, a block's text.
	#[error("offset {offset} does not fit the text of block {block}")]
	Offset {
		/// The text block.
		block: BlockId,
		/// The offending offset, in the units of the side that produced it.
		offset: u32,
	},
	/// The model and the shared document disagree about block structure.
	#[error("the document and the shared document have diverged")]
	Diverged,
	/// yrs returned a root type where it creates nested ones. This would be
	/// a yrs defect.
	#[error("expected a nested shared type")]
	RootType,
	/// Another yrs transaction was open on the shared document.
	#[error("the shared document is busy with another transaction")]
	Busy,
	/// The shared document already has content.
	#[error("the shared document already has content")]
	NotEmpty,
	/// A binary payload could not be decoded.
	#[error("malformed payload: {0}")]
	Decode(#[from] yrs::encoding::read::Error),
	/// A Yjs update could not be integrated.
	#[error("invalid update: {0}")]
	Update(#[from] yrs::error::UpdateError),
	/// Awareness state could not be encoded or applied.
	#[error("awareness: {0}")]
	Awareness(#[from] yrs::sync::awareness::Error),
	/// A peer denied access to the document.
	#[error("permission denied: {0}")]
	PermissionDenied(String),
	/// The server closed the document for this connection.
	#[error("the server closed the document: {0}")]
	Closed(String),
	/// A message type this session does not understand.
	#[error("unsupported message type {0}")]
	UnsupportedMessage(u8),
	/// A scene node has no counterpart in the shared document.
	#[cfg(feature = "scene")]
	#[error("node {0} is not in the shared scene")]
	SceneNode(canvist_scene::NodeId),
	/// A translated scene step was rejected by the scene.
	#[cfg(feature = "scene")]
	#[error(transparent)]
	Scene(#[from] canvist_scene::SceneError),
}
