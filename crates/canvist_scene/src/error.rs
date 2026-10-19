//! Errors raised while changing or decoding a scene.

use canvist_model::JsonError;
use canvist_model::StepError;

use crate::KeyError;
use crate::NodeId;

/// Why a step, transaction, or decode failed. A failed step leaves the
/// scene unchanged.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SceneError {
	/// No node has this id.
	#[error("node {0} does not exist")]
	UnknownNode(NodeId),
	/// A node with this id already exists.
	#[error("node {0} already exists")]
	DuplicateNode(NodeId),
	/// The root cannot be removed, moved, or edited.
	#[error("the root cannot be changed")]
	Root,
	/// The schema has no node type with this name.
	#[error("unknown node type `{0}`")]
	UnknownType(String),
	/// The parent's type does not accept the child's type.
	#[error("a `{child}` cannot be a child of {parent}")]
	NotAccepted {
		/// The parent's type name, or `the root`.
		parent: String,
		/// The child's type name.
		child: String,
	},
	/// The move would make a node its own ancestor.
	#[error("moving {0} there would create a cycle")]
	Cycle(NodeId),
	/// A property value is out of range or does not fit the node's type.
	#[error("invalid `{key}` for node {node}: {reason}")]
	InvalidProp {
		/// The node.
		node: NodeId,
		/// The property name.
		key: String,
		/// What is wrong.
		reason: &'static str,
	},
	/// A text step targeted a node that is not a text frame.
	#[error("node {0} is not a text frame")]
	NotText(NodeId),
	/// The text frame's document is lent to an editor.
	#[error("the text of node {0} is lent to an editor")]
	TextLent(NodeId),
	/// A text step failed inside the frame's document.
	#[error("text step in node {node} failed: {source}")]
	Text {
		/// The text frame.
		node: NodeId,
		/// The text model's error.
		source: StepError,
	},
	/// A fractional key was malformed or could not be generated.
	#[error(transparent)]
	Key(#[from] KeyError),
	/// The client ran out of node ids.
	#[error("node ids exhausted for this client")]
	IdsExhausted,
	/// Scene JSON was malformed.
	#[error("invalid scene JSON at {path}: {reason}")]
	Json {
		/// Where in the document, e.g. `nodes[3].props.fills`.
		path: String,
		/// What is wrong.
		reason: String,
	},
	/// A text frame's document JSON was malformed.
	#[error("invalid text in node {node}: {source}")]
	TextJson {
		/// The text frame.
		node: NodeId,
		/// The text model's error.
		source: JsonError,
	},
}

impl SceneError {
	pub(crate) fn json(path: impl Into<String>, reason: impl Into<String>) -> Self {
		Self::Json {
			path: path.into(),
			reason: reason.into(),
		}
	}
}
