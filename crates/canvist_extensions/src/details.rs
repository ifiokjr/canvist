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
use canvist_editor::Update;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::position;
use canvist_editor::position::Stop;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
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
/// it. Enter in the summary opens it and moves into the content, and
/// Backspace and Delete never join a summary or its content away.
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
	let Some(details) = summary_of(doc, caret.block) else {
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

/// Backspace at the start of a summary unwraps the details, keeping its
/// summary and content as blocks, as Backspace at the start of a code
/// block resets it. At the start of content that is a single block, it
/// moves to the end of the summary: joining the two would leave the
/// details without content.
fn backspace(state: &mut EditorState) -> CommandResult {
	let Some(caret) = state
		.selection()
		.as_caret()
		.filter(|caret| caret.offset == 0)
	else {
		return Ok(None);
	};
	let doc = state.doc();

	if summary_of(doc, caret.block).is_some() {
		return unset(state);
	}

	let Some(content) = position::parent_of(doc, caret.block).filter(|&content| {
		is_named(doc, content, CONTENT) && doc.children(Some(content)) == [caret.block]
	}) else {
		return Ok(None);
	};
	let summary = position::parent_of(doc, content)
		.and_then(|details| doc.children(Some(details)).first().copied());
	let Some(summary) = summary else {
		return Ok(None);
	};
	let end = position::end_of(doc, summary);

	commands::set_selection(state, Selection::caret(end))
}

/// Delete at the end of a summary whose content is a single block moves
/// into the content instead of joining it into the summary, and Delete
/// just before a details selects it instead of joining its summary into
/// the block above, as Delete before an image does. Either join would
/// leave the details without a summary or content.
fn delete(state: &mut EditorState) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let doc = state.doc();

	if caret.offset != position::text_len(doc, caret.block) {
		return Ok(None);
	}

	if let Some(details) = summary_of(doc, caret.block) {
		let content = doc.children(Some(details)).get(1).copied();
		let sole = content
			.map(|content| doc.children(Some(content)))
			.and_then(|children| <[BlockId; 1]>::try_from(children).ok());
		let Some([block]) = sole.filter(|[block]| doc.inline(*block).is_some()) else {
			return Ok(None);
		};

		return commands::set_selection(state, Selection::caret(Point::new(block, 0)));
	}

	let next = match position::next_stop(doc, caret.block) {
		Some(Stop::Text(next)) => summary_of(doc, next.block),
		_ => None,
	};
	let Some(details) = next else {
		return Ok(None);
	};

	commands::set_selection(
		state,
		Selection::Blocks {
			anchor: details,
			head: details,
		},
	)
}

/// Put every details back in TipTap's shape, a summary and then one
/// content block, after an edit the schema alone cannot keep from breaking
/// it, such as deleting a selection that starts above a details and ends
/// in its summary. A missing summary or content is added empty; a second
/// content block or summary moves into the first content.
fn repair(state: &mut EditorState) -> CommandResult {
	let doc = state.doc();
	let schema = doc.schema();
	let kinds = (
		schema.block_type(DETAILS),
		schema.block_type(SUMMARY),
		schema.block_type(CONTENT),
	);
	let (Some(details), Some(summary), Some(content)) = kinds else {
		return Ok(None);
	};
	let shape = [summary, content];
	let broken: Vec<BlockId> = doc
		.iter()
		.filter(|block| block.kind() == details)
		.map(canvist_model::Block::id)
		.filter(|&id| {
			let children = doc.children(Some(id)).iter();
			!children
				.filter_map(|&child| doc.block(child).map(canvist_model::Block::kind))
				.eq(shape)
		})
		.collect();

	if broken.is_empty() {
		return Ok(None);
	}

	let paragraph = schema.default_text_block();
	let selection = state.selection();

	state.change(EditKind::Structure, |tr| {
		for id in broken {
			reshape(tr, id, (summary, content, paragraph))?;
		}

		let selection = position::valid_selection(tr.doc(), selection);
		tr.set_selection(selection);
		Ok(true)
	})
}

/// [`repair`] one details block.
fn reshape(
	tr: &mut canvist_editor::EditorTransaction<'_>,
	details: BlockId,
	(summary, content, paragraph): (BlockTypeId, BlockTypeId, BlockTypeId),
) -> Result<(), EditorError> {
	let kind_at = |tr: &canvist_editor::EditorTransaction<'_>, index: usize| {
		let doc = tr.doc();
		let id = doc.children(Some(details)).get(index).copied()?;
		Some((id, doc.block(id)?.kind()))
	};

	if kind_at(tr, 0).map(|(_, kind)| kind) != Some(summary) {
		let id = tr.allocate_id()?;
		tr.insert_blocks(
			Some(details),
			0,
			vec![BlockTree::text(id, summary, InlineText::new())],
		)?;
	}

	let main = match kind_at(tr, 1) {
		Some((id, kind)) if kind == content => id,
		_ => {
			let id = tr.allocate_id()?;
			tr.insert_blocks(
				Some(details),
				1,
				vec![BlockTree::container(id, content, Vec::new())],
			)?;
			id
		}
	};

	while let Some((extra, kind)) = kind_at(tr, 2) {
		let end = tr.doc().children(Some(main)).len();

		if kind == content {
			let moved = tr.doc().children(Some(extra)).len();
			tr.move_blocks((Some(extra), 0, moved), (Some(main), end))?;
			tr.remove_blocks(Some(details), 2, 1)?;
		} else {
			let text = tr.doc().inline(extra).cloned().unwrap_or_default();
			let id = tr.allocate_id()?;
			tr.insert_blocks(Some(main), end, vec![BlockTree::text(id, paragraph, text)])?;
			tr.remove_blocks(Some(details), 2, 1)?;
		}
	}

	if tr.doc().children(Some(main)).is_empty() {
		let id = tr.allocate_id()?;
		tr.insert_blocks(
			Some(main),
			0,
			vec![BlockTree::text(id, paragraph, InlineText::new())],
		)?;
	}

	Ok(())
}

/// Whether `id` is a block of the type `name`.
fn is_named(doc: &canvist_model::Document, id: BlockId, name: &str) -> bool {
	doc.block(id)
		.is_some_and(|block| &*doc.schema().block(block.kind()).name == name)
}

/// The details whose summary is `id`, if `id` is a summary.
fn summary_of(doc: &canvist_model::Document, id: BlockId) -> Option<BlockId> {
	position::parent_of(doc, id).filter(|_| is_named(doc, id, SUMMARY))
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

	/// Only an edit that changes a child list can break a details' shape,
	/// so typing never pays for the walk [`repair`] takes.
	fn after_update(&self, state: &mut EditorState, update: &Update) -> CommandResult {
		if !update.commit.structural {
			return Ok(None);
		}

		repair(state)
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![
			("Enter", command(enter)),
			("Backspace", command(backspace)),
			("Delete", command(delete)),
		])
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
