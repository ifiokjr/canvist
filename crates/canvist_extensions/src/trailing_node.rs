//! A trailing paragraph.

use canvist_core::CommandResult;
use canvist_core::EditKind;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Update;
use canvist_model::BlockTree;
use canvist_model::InlineText;

/// Keeps an empty paragraph at the end of the document whenever the last
/// block is anything else, so there is always somewhere to click and type
/// after a list, quote, table, or divider.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrailingNode;

impl Extension for TrailingNode {
	fn name(&self) -> &'static str {
		"trailingNode"
	}

	fn after_update(&self, state: &mut EditorState, _update: &Update) -> CommandResult {
		let doc = state.doc();
		let paragraph = doc.schema().default_text_block();
		let last = doc.children(None).last().and_then(|&id| doc.block(id));

		if last.is_some_and(|block| block.kind() == paragraph) {
			return Ok(None);
		}

		let selection = state.selection();
		let index = doc.children(None).len();

		state.change(EditKind::Structure, |tr| {
			let id = tr.allocate_id()?;
			let block = BlockTree::text(id, paragraph, InlineText::new());
			tr.insert_blocks(None, index, vec![block])?;
			tr.set_selection(selection);
			Ok(true)
		})
	}
}

impl canvist_core::TypedExtension for TrailingNode {
	type Handle = canvist_core::Registration;

	fn handle(&self, registration: canvist_core::Registration) -> canvist_core::Registration {
		registration
	}
}
