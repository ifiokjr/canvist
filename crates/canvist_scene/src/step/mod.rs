//! Steps: the only way to change a scene.
//!
//! Every step is invertible and shaped after one operation on the Yjs
//! document (see the [crate docs](crate#the-yjs-shape)): inserting or
//! deleting entries of the flat node map, setting a node's position
//! register, setting one property register, or one text-model step inside
//! a text frame. A collaboration binding translates them one to one.

mod ops;

use crate::FracKey;
use crate::NodeId;
use crate::NodeRecord;
use crate::Prop;
use crate::Scene;
use crate::SceneError;

/// An atomic, invertible scene change.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
	/// Insert nodes. Records come parents first; a record's parent is an
	/// existing node, the root, or an earlier record.
	InsertNodes {
		/// The nodes to insert.
		nodes: Vec<NodeRecord>,
	},
	/// Remove nodes with their descendants and text documents.
	RemoveNodes {
		/// The nodes to remove.
		ids: Vec<NodeId>,
	},
	/// Reparent or reorder a node, keeping its id, props, and descendants.
	Move {
		/// The node.
		node: NodeId,
		/// The new parent.
		parent: NodeId,
		/// The new key among the new parent's children.
		key: FracKey,
	},
	/// Set one property.
	SetProp {
		/// The node.
		node: NodeId,
		/// The property and its new value.
		prop: Prop,
	},
	/// Apply one text-model step to a text frame's document.
	Text {
		/// The text frame.
		node: NodeId,
		/// The step.
		step: canvist_model::Step,
	},
}

/// What applying a step produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
	/// The step that undoes this one.
	pub inverse: Step,
	/// Nodes whose own props, position, or text changed, plus parents whose
	/// child lists changed.
	pub touched: Vec<NodeId>,
	/// Whether the tree's structure (any child list) changed.
	pub structural: bool,
}

impl Step {
	/// Apply this step to `scene`. On error the scene is unchanged.
	pub fn apply(self, scene: &mut Scene) -> Result<Applied, SceneError> {
		let applied = match self {
			Self::InsertNodes { nodes } => ops::insert_nodes(scene, nodes),
			Self::RemoveNodes { ids } => ops::remove_nodes(scene, &ids),
			Self::Move { node, parent, key } => ops::move_node(scene, node, parent, key),
			Self::SetProp { node, prop } => ops::set_prop(scene, node, prop),
			Self::Text { node, step } => ops::text(scene, node, step),
		}?;

		scene.bump_version();

		for &id in &applied.touched {
			scene.touch(id);
		}

		Ok(applied)
	}

	/// The nodes this step names directly.
	#[must_use]
	pub fn nodes(&self) -> Vec<NodeId> {
		match self {
			Self::InsertNodes { nodes } => nodes.iter().map(|record| record.id).collect(),
			Self::RemoveNodes { ids } => ids.clone(),
			Self::Move { node, .. } | Self::SetProp { node, .. } | Self::Text { node, .. } => {
				vec![*node]
			}
		}
	}
}
