//! Details: a summary that opens and closes the content below it.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::CommandSpec;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Platform;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::position;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::BlockTree;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

use crate::resolve;

const DETAILS: &str = "details";
const SUMMARY: &str = "detailsSummary";
const CONTENT: &str = "detailsContent";

/// TipTap's Details: a `details` block holding a `detailsSummary` text
/// block and a `detailsContent` block of content. Its `open` attribute says
/// whether the content shows; layout draws a closed one as its summary
/// beside a `▸`.
///
/// `setDetails` wraps the selected blocks, `unsetDetails` turns the summary
/// back into a paragraph above them, and `toggleDetailsOpen` opens or closes
/// it. Enter in the summary opens it and moves into the content.
#[derive(Clone, Copy, Debug, Default)]
pub struct Details;

/// Typed access to [`Details`].
#[derive(Clone, Copy, Debug)]
pub struct DetailsHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl DetailsHandle {
	/// Wrap the selected blocks in an open details block with an empty
	/// summary, and put the caret in the summary.
	#[must_use]
	pub fn set(self) -> Command {
		command(set)
	}

	/// Replace the details around the selection with its summary, as a
	/// paragraph, and its content.
	#[must_use]
	pub fn unset(self) -> Command {
		command(unset)
	}

	/// Open the details around the selection, or close it.
	#[must_use]
	pub fn toggle_open(self) -> Command {
		command(toggle_open)
	}

	/// Whether the details around the selection is open, if there is one.
	#[must_use]
	pub fn is_open(self, state: &EditorState) -> Option<bool> {
		open_state(state)
	}
}

/// The details block around the selection head.
fn around(state: &EditorState) -> Option<BlockId> {
	resolve::around(state, DETAILS)
}

fn open_state(state: &EditorState) -> Option<bool> {
	let details = around(state)?;
	let open = state.doc().block(details)?.attrs().get("open");
	Some(open.and_then(Value::as_bool).unwrap_or(false))
}

fn set(state: &mut EditorState) -> CommandResult {
	let details = resolve::block(state, DETAILS, DETAILS)?;
	let summary = resolve::block(state, DETAILS, SUMMARY)?;
	let content = resolve::block(state, DETAILS, CONTENT)?;
	let doc = state.doc();
	let Some((parent, range)) = selected_siblings(state) else {
		return Ok(None);
	};
	let parent_kind = parent
		.and_then(|id| doc.block(id))
		.map(canvist_model::Block::kind);
	let movable = doc.children(parent)[range.clone()].iter().all(|&id| {
		doc.block(id)
			.is_some_and(|block| doc.schema().allows_child(Some(content), block.kind()))
	});

	if !doc.schema().allows_child(parent_kind, details) || !movable {
		return Ok(None);
	}

	state.change(EditKind::Structure, |tr| {
		let ids = (tr.allocate_id()?, tr.allocate_id()?, tr.allocate_id()?);
		let tree = BlockTree::container(
			ids.0,
			details,
			vec![
				BlockTree::text(ids.1, summary, InlineText::new()),
				BlockTree::container(ids.2, content, Vec::new()),
			],
		)
		.with_attrs(Attrs::new().with("open", true));

		tr.insert_blocks(parent, range.start, vec![tree])?;
		tr.move_blocks((parent, range.start + 1, range.len()), (Some(ids.2), 0))?;
		tr.set_selection(Selection::caret(Point::new(ids.1, 0)));
		Ok(true)
	})
}

/// The selected sibling blocks: their parent and index range.
fn selected_siblings(state: &EditorState) -> Option<(Option<BlockId>, std::ops::Range<usize>)> {
	let doc = state.doc();
	let range = position::text_range(doc, state.selection())?;
	position::sibling_range(doc, range.from.block, range.to.block)
}

fn unset(state: &mut EditorState) -> CommandResult {
	let paragraph = state.schema().default_text_block();
	let doc = state.doc();
	let Some(details) = around(state) else {
		return Ok(None);
	};
	let parent = position::parent_of(doc, details);
	let Some(index) = doc.index_in_parent(details) else {
		return Ok(None);
	};
	let children = doc.children(Some(details)).to_vec();
	let summary = children
		.first()
		.and_then(|&id| doc.inline(id))
		.filter(|text| !text.is_empty())
		.cloned();
	let content = children.get(1).copied();
	let moved = content.map_or(0, |id| doc.children(Some(id)).len());

	state.change(EditKind::Structure, |tr| {
		if let Some(content) = content.filter(|_| moved > 0) {
			tr.move_blocks((Some(content), 0, moved), (parent, index + 1))?;
		}

		let mut caret = None;

		if let Some(summary) = summary {
			let id = tr.allocate_id()?;
			tr.insert_blocks(
				parent,
				index + 1,
				vec![BlockTree::text(id, paragraph, summary)],
			)?;
			caret = Some(Point::new(id, 0));
		}

		tr.remove_blocks(parent, index, 1)?;
		let first = tr.doc().children(parent).get(index).copied();
		let caret = caret.or_else(|| first.map(|block| Point::new(block, 0)));

		if let Some(caret) = caret {
			tr.set_selection(position::valid_selection(tr.doc(), Selection::caret(caret)));
		}

		Ok(true)
	})
}

fn toggle_open(state: &mut EditorState) -> CommandResult {
	let Some(details) = around(state) else {
		return Ok(None);
	};
	let open = open_state(state).unwrap_or(false);
	let summary = state.doc().children(Some(details)).first().copied();
	set_open(state, details, !open, summary)
}

/// Open or close `details`. Closing moves a caret out of the hidden content
/// to the end of the summary.
fn set_open(
	state: &mut EditorState,
	details: BlockId,
	open: bool,
	summary: Option<BlockId>,
) -> CommandResult {
	let Some(block) = state.doc().block(details) else {
		return Ok(None);
	};
	let kind = block.kind();
	let attrs = block.attrs().clone().with("open", open);
	let head = match state.selection() {
		Selection::Text { head, .. } => head.block,
		Selection::Blocks { head, .. } => head,
	};
	let in_summary = Some(head) == summary;
	let caret = summary
		.filter(|_| !open && !in_summary)
		.map(|summary| position::end_of(state.doc(), summary));

	state.change(EditKind::Structure, |tr| {
		tr.set_block(details, kind, attrs)?;

		if let Some(caret) = caret {
			tr.set_selection(Selection::caret(caret));
		}

		Ok(true)
	})
}

/// Enter in a summary: open the details and move to the start of its
/// content, adding a paragraph when the content is empty.
fn enter(state: &mut EditorState) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let doc = state.doc();
	let is_summary = doc
		.block(caret.block)
		.is_some_and(|block| &*doc.schema().block(block.kind()).name == SUMMARY);
	let Some(details) = position::parent_of(doc, caret.block).filter(|_| is_summary) else {
		return Ok(None);
	};
	let Some(&content) = doc.children(Some(details)).get(1) else {
		return Ok(None);
	};
	let first = doc.children(Some(content)).first().copied();
	let paragraph = doc.schema().default_text_block();
	let attrs = doc
		.block(details)
		.map(|block| block.attrs().clone().with("open", true));
	let kind = doc.block(details).map(canvist_model::Block::kind);
	let (Some(attrs), Some(kind)) = (attrs, kind) else {
		return Ok(None);
	};

	state.change(EditKind::Structure, |tr| {
		tr.set_block(details, kind, attrs)?;
		let target = if let Some(first) = first {
			first
		} else {
			let id = tr.allocate_id()?;
			tr.insert_blocks(
				Some(content),
				0,
				vec![BlockTree::text(id, paragraph, InlineText::new())],
			)?;
			id
		};
		let start = first_text(tr.doc(), target);
		tr.set_selection(Selection::caret(Point::new(start, 0)));
		Ok(true)
	})
}

/// The first text block in `id`, or `id` itself.
fn first_text(doc: &canvist_model::Document, id: BlockId) -> BlockId {
	let mut block = id;

	while let Some(&first) = doc.children(Some(block)).first() {
		block = first;
	}

	block
}

impl Extension for Details {
	fn name(&self) -> &'static str {
		DETAILS
	}

	/// Above the default, so Enter in a summary opens it before the
	/// standard keys split the block.
	fn priority(&self) -> i32 {
		101
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema
			.block(
				BlockSpec::container(DETAILS, &[SUMMARY, CONTENT])
					.attrs(Attrs::new().with("open", false)),
			)
			.block(BlockSpec::text(SUMMARY).groups(&[]))
			.block(BlockSpec::container(CONTENT, &["block"]).groups(&[]))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let open = Field::optional("open", ValueType::Bool, false, "Whether the content shows.");
		vec![
			NodeMeta::new(NodeKind::Block, DETAILS).attr(open),
			NodeMeta::new(NodeKind::Block, SUMMARY),
			NodeMeta::new(NodeKind::Block, CONTENT),
		]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![
			CommandSpec::simple(
				"setDetails",
				"Wrap the selected blocks in an open details block with an empty summary.",
				command(set),
			),
			CommandSpec::simple(
				"unsetDetails",
				"Replace the details around the selection with its summary and content.",
				command(unset),
			),
			CommandSpec::simple(
				"toggleDetailsOpen",
				"Open the details around the selection, or close it.",
				command(toggle_open),
			),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"detailsOpen",
			"Whether the details around the selection is open, or null outside one.",
			ValueType::nullable(ValueType::Bool),
			|state, _| Ok(open_state(state).map_or(Value::Null, Value::Bool)),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Enter", command(enter))])
	}
}

impl TypedExtension for Details {
	type Handle = DetailsHandle;

	fn handle(&self, registration: Registration) -> DetailsHandle {
		DetailsHandle {
			_registration: registration,
		}
	}
}
