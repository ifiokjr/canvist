//! Moving blocks among their siblings: what a drag handle, or Alt and an
//! arrow key, does.

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::Platform;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::position;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Schema;
use canvist_model::Selection;

/// The commands behind TipTap's `DragHandle`.
///
/// The handle itself is host UI: the host places it with the layout's
/// `block_rect` and finds where a drop lands with `hit_block`. These
/// commands do the move. `moveBlockUp` and `moveBlockDown` (Alt-↑ and
/// Alt-↓) swap the selected blocks with their neighbour, reaching for the
/// enclosing block when they are already at the edge of their container,
/// so a lone paragraph in a list item moves the item. `moveBlocksTo` puts
/// them before sibling `index`, as a drop indicator does.
#[derive(Clone, Copy, Debug, Default)]
pub struct DragHandle;

/// Typed access to [`DragHandle`].
#[derive(Clone, Copy, Debug)]
pub struct DragHandleHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl DragHandleHandle {
	/// Move the selected blocks up past their previous sibling.
	#[must_use]
	pub fn up(self) -> Command {
		command(|state| step(state, false))
	}

	/// Move the selected blocks down past their next sibling.
	#[must_use]
	pub fn down(self) -> Command {
		command(|state| step(state, true))
	}

	/// Move the selected blocks to just before sibling `index`, or to the
	/// end of their container when `index` is past the last.
	#[must_use]
	pub fn move_to(self, index: usize) -> Command {
		command(move |state| move_to(state, index))
	}
}

/// The selected sibling blocks: their parent and index range. A block
/// selection, as a click on a drag handle or a horizontal rule makes, moves
/// the blocks it covers.
fn selected(state: &EditorState) -> Option<(Option<BlockId>, std::ops::Range<usize>)> {
	let doc = state.doc();
	let (first, last) = match state.selection() {
		Selection::Blocks { anchor, head }
			if position::compare_blocks(doc, anchor, head).is_gt() =>
		{
			(head, anchor)
		}
		Selection::Blocks { anchor, head } => (anchor, head),
		selection @ Selection::Text { .. } => {
			let range = position::text_range(doc, selection)?;
			(range.from.block, range.to.block)
		}
	};

	position::sibling_range(doc, first, last)
}

/// `range` of `parent`'s children, or the nearest enclosing block that can
/// move `down` (or up) when `range` is at its container's edge.
fn movable(
	doc: &Document,
	(mut parent, mut range): (Option<BlockId>, std::ops::Range<usize>),
	down: bool,
) -> Option<(Option<BlockId>, std::ops::Range<usize>)> {
	loop {
		let free = if down {
			range.end < doc.children(parent).len()
		} else {
			range.start > 0
		};

		if free {
			return Some((parent, range));
		}

		let container = parent?;
		let index = doc.index_in_parent(container)?;
		parent = position::parent_of(doc, container);
		range = index..index + 1;
	}
}

fn step(state: &mut EditorState, down: bool) -> CommandResult {
	let Some((parent, range)) = selected(state).and_then(|found| movable(state.doc(), found, down))
	else {
		return Ok(None);
	};
	let to = if down {
		range.start + 1
	} else {
		range.start - 1
	};

	shift(state, parent, range, to)
}

fn move_to(state: &mut EditorState, index: usize) -> CommandResult {
	let Some((parent, range)) = selected(state) else {
		return Ok(None);
	};
	let index = index.min(state.doc().children(parent).len());

	if (range.start..=range.end).contains(&index) {
		return Ok(None);
	}

	// The target is counted after the moving blocks are taken out.
	let to = if index > range.end {
		index - range.len()
	} else {
		index
	};

	shift(state, parent, range, to)
}

/// Move `range` of `parent`'s children to `to`, counted after removal,
/// keeping the selection on them.
fn shift(
	state: &mut EditorState,
	parent: Option<BlockId>,
	range: std::ops::Range<usize>,
	to: usize,
) -> CommandResult {
	let selection = state.selection();

	state.change(EditKind::Structure, |tr| {
		tr.move_blocks((parent, range.start, range.len()), (parent, to))?;
		tr.set_selection(selection);
		Ok(true)
	})
}

impl Extension for DragHandle {
	fn name(&self) -> &'static str {
		"dragHandle"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let to = CommandSpec::new(
			"moveBlocksTo",
			"Move the selected blocks to just before a sibling, by its index, as a drop does.",
			|args| {
				let index = usize::try_from(args.int("index")?).map_err(|_| {
					EditorError::InvalidArgs {
						name: "moveBlocksTo".to_owned(),
						reason: "`index` must not be negative".to_owned(),
					}
				})?;
				Ok(command(move |state| move_to(state, index)))
			},
		)
		.arg(Field::required(
			"index",
			ValueType::Int,
			"The sibling to move before, from zero; past the last moves to the end.",
		));

		Ok(vec![
			CommandSpec::simple(
				"moveBlockUp",
				"Move the selected blocks up past their previous sibling.",
				command(|state| step(state, false)),
			),
			CommandSpec::simple(
				"moveBlockDown",
				"Move the selected blocks down past their next sibling.",
				command(|state| step(state, true)),
			),
			to,
		])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![
			("Alt-ArrowUp", command(|state| step(state, false))),
			("Alt-ArrowDown", command(|state| step(state, true))),
		])
	}
}

impl TypedExtension for DragHandle {
	type Handle = DragHandleHandle;

	fn handle(&self, registration: Registration) -> DragHandleHandle {
		DragHandleHandle {
			_registration: registration,
		}
	}
}
