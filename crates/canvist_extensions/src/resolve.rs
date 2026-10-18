//! Looking up types and attributes at run time.
//!
//! Handles exist before the schema does, so their commands resolve block
//! and mark types when they run. Run against an editor that lacks the
//! extension, they fail with [`EditorError::MissingType`] instead of
//! touching the wrong type.

#![cfg_attr(
	not(feature = "starter-kit"),
	allow(
		dead_code,
		reason = "each extension uses a different subset of these helpers"
	)
)]

use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::position;
use canvist_model::BlockId;
use canvist_model::BlockTypeId;
use canvist_model::MarkTypeId;
use canvist_model::Selection;
use canvist_model::Value;

/// The block type `name`, which extension `extension` needs.
pub(crate) fn block(
	state: &EditorState,
	extension: &'static str,
	name: &'static str,
) -> Result<BlockTypeId, EditorError> {
	canvist_editor::block_type(state.schema(), extension, name)
}

/// The mark type `name`, which extension `extension` needs.
pub(crate) fn mark(
	state: &EditorState,
	extension: &'static str,
	name: &'static str,
) -> Result<MarkTypeId, EditorError> {
	canvist_editor::mark_type(state.schema(), extension, name)
}

/// The block holding the selection head (or the first selected block).
fn head_block(state: &EditorState) -> BlockId {
	match state.selection() {
		Selection::Text { head, .. } => head.block,
		Selection::Blocks { anchor, .. } => anchor,
	}
}

/// The nearest block of type `name` around the selection head.
pub(crate) fn around(state: &EditorState, name: &str) -> Option<BlockId> {
	let kind = state.schema().block_type(name)?;
	let doc = state.doc();

	position::ancestors_or_self(doc, head_block(state))
		.find(|&id| doc.block(id).is_some_and(|block| block.kind() == kind))
}

/// Attribute `key` of the nearest block of type `name` around the head.
pub(crate) fn attr_around(state: &EditorState, name: &str, key: &str) -> Option<Value> {
	let id = around(state, name)?;
	state.doc().block(id)?.attrs().get(key).cloned()
}

/// Whether mark `name` is active at the selection.
pub(crate) fn mark_active(state: &EditorState, name: &str) -> bool {
	state
		.schema()
		.mark_type(name)
		.is_some_and(|kind| canvist_editor::commands::is_mark_active(state, kind))
}
