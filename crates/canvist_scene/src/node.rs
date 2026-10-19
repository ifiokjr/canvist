//! Scene nodes and the records steps carry.

use canvist_model::BlockTree;

use crate::FracKey;
use crate::NodeId;
use crate::NodeTypeId;
use crate::Props;

/// One node in a [`Scene`](crate::Scene).
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
	pub(crate) id: NodeId,
	pub(crate) kind: NodeTypeId,
	pub(crate) parent: NodeId,
	pub(crate) key: FracKey,
	pub(crate) props: Props,
	pub(crate) rev: u64,
}

impl Node {
	/// The node's id.
	#[must_use]
	pub fn id(&self) -> NodeId {
		self.id
	}

	/// The node's type.
	#[must_use]
	pub fn kind(&self) -> NodeTypeId {
		self.kind
	}

	/// The parent, or [`NodeId::ROOT`] for pages.
	#[must_use]
	pub fn parent(&self) -> NodeId {
		self.parent
	}

	/// The fractional key ordering this node among its siblings.
	#[must_use]
	pub fn key(&self) -> &FracKey {
		&self.key
	}

	/// The node's properties.
	#[must_use]
	pub fn props(&self) -> &Props {
		&self.props
	}

	/// The scene version at which this node last changed. Caches key on it.
	#[must_use]
	pub fn rev(&self) -> u64 {
		self.rev
	}
}

/// Everything needed to insert a node: the payload of
/// [`Step::InsertNodes`](crate::Step::InsertNodes) and the inverse of
/// [`Step::RemoveNodes`](crate::Step::RemoveNodes).
#[derive(Clone, Debug, PartialEq)]
pub struct NodeRecord {
	/// The node's id.
	pub id: NodeId,
	/// The node's type.
	pub kind: NodeTypeId,
	/// The parent, or [`NodeId::ROOT`] for pages.
	pub parent: NodeId,
	/// The key among the parent's children.
	pub key: FracKey,
	/// The properties.
	pub props: Props,
	/// A text frame's blocks; `None` starts the frame with one empty
	/// paragraph. Must be `None` for other types.
	pub text: Option<Vec<BlockTree>>,
}

impl NodeRecord {
	/// A record with default props and no text.
	#[must_use]
	pub fn new(id: NodeId, kind: NodeTypeId, parent: NodeId, key: FracKey) -> Self {
		Self {
			id,
			kind,
			parent,
			key,
			props: Props::default(),
			text: None,
		}
	}

	/// Replace the props.
	#[must_use]
	pub fn with_props(mut self, props: Props) -> Self {
		self.props = props;
		self
	}

	/// Set a text frame's blocks.
	#[must_use]
	pub fn with_text(mut self, blocks: Vec<BlockTree>) -> Self {
		self.text = Some(blocks);
		self
	}
}
