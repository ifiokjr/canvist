//! List commands: split, indent, outdent, join, and toggle.
//!
//! Lists are containers of items, and items are containers of blocks. The
//! commands are generic over both types, so bullet, ordered, and task lists
//! (and any list an extension invents) share one implementation.

use std::ops::Range;

use canvist_model::BlockId;
use canvist_model::BlockTypeId;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::TreeContent;

use super::CommandResult;
use crate::EditKind;
use crate::EditorError;
use crate::EditorState;
use crate::EditorTransaction;
use crate::position;
use crate::position::parent_of;
use crate::structure;
use crate::structure::kind_of;

/// A list type and the item type it holds, e.g. `bulletList` and `listItem`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListType {
	/// The list container.
	pub list: BlockTypeId,
	/// The item container inside it.
	pub item: BlockTypeId,
}

/// The first and last blocks a selection touches.
fn selection_ends(doc: &Document, selection: Selection) -> Option<(BlockId, BlockId)> {
	match selection {
		Selection::Text { .. } => {
			let range = position::text_range(doc, selection)?;
			Some((range.from.block, range.to.block))
		}
		Selection::Blocks { anchor, head } => {
			if position::compare_blocks(doc, anchor, head).is_gt() {
				Some((head, anchor))
			} else {
				Some((anchor, head))
			}
		}
	}
}

/// The items of type `item` the selection covers: their list and the
/// index range of the items in it.
fn item_range(
	doc: &Document,
	selection: Selection,
	item: BlockTypeId,
) -> Option<(BlockId, Range<usize>)> {
	let (first, last) = selection_ends(doc, selection)?;
	let (parent, range) = position::sibling_range(doc, first, last)?;
	let parent = parent?;
	let children = doc.children(Some(parent));
	let all_items = children[range.clone()]
		.iter()
		.all(|&child| kind_of(doc, child) == Some(item));

	if all_items {
		return Some((parent, range));
	}

	let found =
		position::ancestors_or_self(doc, parent).find(|&id| kind_of(doc, id) == Some(item))?;
	let owner = parent_of(doc, found)?;
	let index = doc.index_in_parent(found)?;
	Some((owner, index..index + 1))
}

/// Enter inside a list item: split the item at the caret.
///
/// In an empty item, Enter leaves the list instead: the item is lifted one
/// level, or out of the list entirely at the top level.
pub fn split_list_item(state: &mut EditorState, item: BlockTypeId) -> CommandResult {
	let Some(range) = position::text_range(state.doc(), state.selection()) else {
		return Ok(None);
	};
	let Some(item_id) = parent_of(state.doc(), range.from.block)
		.filter(|&id| kind_of(state.doc(), id) == Some(item))
	else {
		return Ok(None);
	};

	state.change(EditKind::Structure, |tr| {
		let at = structure::delete_range(tr, range.from, range.to)?;
		split_item_at(tr, item_id, at, item)
	})
}

fn split_item_at(
	tr: &mut EditorTransaction<'_>,
	item_id: BlockId,
	at: Point,
	item: BlockTypeId,
) -> Result<bool, EditorError> {
	let doc = tr.doc();
	let Some(list) = parent_of(doc, item_id) else {
		return Ok(false);
	};
	let block_index = structure::index_of(doc, at.block)?;
	let len = position::text_len(doc, at.block);
	let is_last = block_index + 1 == doc.children(Some(item_id)).len();

	if len == 0 && is_last && block_index == 0 {
		let index = structure::index_of(doc, item_id)?;
		let lifted = lift_items(tr, list, index..index + 1, item)?;
		tr.set_selection(Selection::caret(at));

		return Ok(lifted);
	}

	let (caret, moved_from) = if len == 0 && is_last {
		(Point::new(at.block, 0), block_index)
	} else if at.offset == len {
		let Some(kind) = structure::default_text_type(doc, Some(item_id)) else {
			return Ok(false);
		};
		let fresh = structure::fresh_block(tr, kind)?;
		let caret = Point::new(fresh.id, 0);
		tr.insert_blocks(Some(item_id), block_index + 1, vec![fresh])?;
		(caret, block_index + 1)
	} else {
		let block = tr.doc().block(at.block);
		let (kind, attrs) = block.map(|b| (b.kind(), b.attrs().clone())).unzip();
		let (Some(kind), Some(attrs)) = (kind, attrs) else {
			return Ok(false);
		};
		(tr.split(at, kind, attrs)?, block_index + 1)
	};

	let new_item = structure::fresh_block(tr, item)?;
	let new_id = new_item.id;
	let index = structure::index_of(tr.doc(), item_id)?;
	tr.insert_blocks(Some(list), index + 1, vec![new_item])?;
	let count = tr.doc().children(Some(item_id)).len() - moved_from;
	tr.move_blocks((Some(item_id), moved_from, count), (Some(new_id), 0))?;
	tr.set_selection(Selection::caret(caret));
	Ok(true)
}

/// Tab: nest the selected items inside the item before them.
pub fn sink_list_item(state: &mut EditorState, item: BlockTypeId) -> CommandResult {
	let selection = state.selection();
	let doc = state.doc();
	let Some((list, range)) = item_range(doc, selection, item) else {
		return Ok(None);
	};

	if range.start == 0 {
		return Ok(None);
	}

	let Some(list_kind) = kind_of(doc, list) else {
		return Ok(None);
	};
	let previous = doc.children(Some(list))[range.start - 1];
	let existing = doc
		.children(Some(previous))
		.last()
		.copied()
		.filter(|&last| kind_of(doc, last) == Some(list_kind));

	if existing.is_none() && !structure::allows(doc, Some(previous), list_kind) {
		return Ok(None);
	}

	state.change(EditKind::Structure, |tr| {
		let target = match existing {
			Some(sublist) => sublist,
			None => append_copy(tr, list, previous)?,
		};

		let end = tr.doc().children(Some(target)).len();
		tr.move_blocks((Some(list), range.start, range.len()), (Some(target), end))?;
		tr.set_selection(selection);
		Ok(true)
	})
}

/// Append an empty copy of `list` to the end of `item`.
fn append_copy(
	tr: &mut EditorTransaction<'_>,
	list: BlockId,
	item: BlockId,
) -> Result<BlockId, EditorError> {
	let copy = structure::empty_copy(tr, list)?;
	let id = copy.id;
	let end = tr.doc().children(Some(item)).len();
	tr.insert_blocks(Some(item), end, vec![copy])?;
	Ok(id)
}

/// Shift-Tab: lift the selected items one level. Nested items become items
/// of the outer list; top-level items leave the list and their content
/// takes their place.
pub fn lift_list_item(state: &mut EditorState, item: BlockTypeId) -> CommandResult {
	let selection = state.selection();
	let Some((list, range)) = item_range(state.doc(), selection, item) else {
		return Ok(None);
	};

	state.change(EditKind::Structure, |tr| {
		let lifted = lift_items(tr, list, range, item)?;
		tr.set_selection(selection);
		Ok(lifted)
	})
}

fn lift_items(
	tr: &mut EditorTransaction<'_>,
	list: BlockId,
	range: Range<usize>,
	item: BlockTypeId,
) -> Result<bool, EditorError> {
	let doc = tr.doc();
	let outer = parent_of(doc, list);

	if let Some(outer_item) = outer.filter(|&id| kind_of(doc, id) == Some(item)) {
		lift_nested(tr, list, range, outer_item)?;

		return Ok(true);
	}

	let items = doc.children(Some(list))[range.clone()].to_vec();
	let fits = items.iter().all(|&id| {
		doc.children(Some(id))
			.iter()
			.filter_map(|&child| kind_of(doc, child))
			.all(|kind| structure::allows(doc, outer, kind))
	});

	if !fits {
		return Ok(false);
	}

	let list_index = structure::index_of(doc, list)?;
	structure::split_container(tr, list, range.end)?;
	let mut at = list_index + 1;

	for id in items {
		let count = tr.doc().children(Some(id)).len();
		tr.move_blocks((Some(id), 0, count), (outer, at))?;
		at += count;
	}

	tr.remove_blocks(Some(list), range.start, range.len())?;
	structure::prune_empty(tr, Some(list))?;
	Ok(true)
}

/// Move nested items into the outer list after their parent item. Items
/// that followed them stay nested, under the last lifted item.
fn lift_nested(
	tr: &mut EditorTransaction<'_>,
	list: BlockId,
	range: Range<usize>,
	outer_item: BlockId,
) -> Result<(), EditorError> {
	let doc = tr.doc();
	let outer_list = parent_of(doc, outer_item);
	let outer_index = structure::index_of(doc, outer_item)?;
	let len = doc.children(Some(list)).len();
	let list_kind = kind_of(doc, list);

	if range.end < len {
		let last_lifted = doc.children(Some(list))[range.end - 1];
		let existing = doc
			.children(Some(last_lifted))
			.last()
			.copied()
			.filter(|&child| kind_of(doc, child) == list_kind);

		let target = match existing {
			Some(sublist) => sublist,
			None => append_copy(tr, list, last_lifted)?,
		};

		let end = tr.doc().children(Some(target)).len();
		tr.move_blocks(
			(Some(list), range.end, len - range.end),
			(Some(target), end),
		)?;
	}

	tr.move_blocks(
		(Some(list), range.start, range.len()),
		(outer_list, outer_index + 1),
	)?;
	structure::prune_empty(tr, Some(list))
}

/// Backspace at the start of a list item: join its text into the previous
/// item, or lift the item when it is the first one or the previous item ends
/// in a nested list.
pub fn join_list_item_backward(state: &mut EditorState, item: BlockTypeId) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let doc = state.doc();
	let item_id = parent_of(doc, caret.block).filter(|&id| kind_of(doc, id) == Some(item));
	let Some(item_id) = item_id.filter(|_| caret.offset == 0) else {
		return Ok(None);
	};

	if doc.children(Some(item_id)).first() != Some(&caret.block) {
		return Ok(None);
	}

	let Some(list) = parent_of(doc, item_id) else {
		return Ok(None);
	};
	let index = doc.index_in_parent(item_id).unwrap_or_default();
	let target = index
		.checked_sub(1)
		.map(|previous| doc.children(Some(list))[previous])
		.map(|previous| (previous, position::last_descendant(doc, previous)))
		.filter(|&(previous, last)| {
			doc.inline(last).is_some() && parent_of(doc, last) == Some(previous)
		})
		.map(|(_, last)| last);

	state.change(EditKind::Structure, |tr| {
		let Some(target) = target else {
			let lifted = lift_items(tr, list, index..index + 1, item)?;
			tr.set_selection(Selection::caret(caret));
			return Ok(lifted);
		};
		let meet = structure::merge_text(tr, target, caret.block)?;
		tr.set_selection(Selection::caret(meet));
		Ok(true)
	})
}

/// Toggle the selection into or out of a `target` list.
///
/// `known` lists every list type the editor has, so a bullet list can turn
/// into a task list (whose items differ) instead of nesting inside it.
/// Inside a `target` list the items are lifted out; inside another known
/// list it is converted in place; elsewhere the selected blocks are wrapped
/// and joined with any adjacent list of the same type.
pub fn toggle_list(state: &mut EditorState, target: ListType, known: &[ListType]) -> CommandResult {
	let selection = state.selection();
	let doc = state.doc();
	let current = std::iter::once(&target)
		.chain(known)
		.filter_map(|list_type| {
			let (list, range) = item_range(doc, selection, list_type.item)?;
			(kind_of(doc, list) == Some(list_type.list)).then_some((*list_type, list, range))
		})
		.max_by_key(|(_, list, _)| position::ancestors_or_self(doc, *list).count());

	match current {
		Some((found, list, range)) if found == target => {
			state.change(EditKind::Structure, |tr| {
				let lifted = lift_items(tr, list, range, found.item)?;
				tr.set_selection(selection);
				Ok(lifted)
			})
		}

		Some((found, list, _)) if found.item == target.item => {
			let attrs = structure::default_attrs(doc, target.list);
			state.change(EditKind::Structure, |tr| {
				tr.set_block(list, target.list, attrs)?;
				Ok(true)
			})
		}

		Some((found, list, _)) => convert_list(state, list, found, target),
		None => wrap_in_list(state, target),
	}
}

/// Rebuild `list` as a `target` list, keeping every block id so the
/// selection and layout caches stay valid.
fn convert_list(
	state: &mut EditorState,
	list: BlockId,
	found: ListType,
	target: ListType,
) -> CommandResult {
	let selection = state.selection();
	let doc = state.doc();
	let Some(mut tree) = doc.tree(list) else {
		return Ok(None);
	};
	let schema = doc.schema();
	tree.kind = target.list;
	tree.attrs = structure::default_attrs(doc, target.list);

	if let TreeContent::Blocks(items) = &mut tree.content {
		for entry in items.iter_mut().filter(|entry| entry.kind == found.item) {
			entry.kind = target.item;
			entry.attrs = structure::default_attrs(doc, target.item);
		}

		let fits = items.iter().all(|entry| {
			match &entry.content {
				TreeContent::Blocks(children) => {
					children
						.iter()
						.all(|child| schema.allows_child(Some(entry.kind), child.kind))
				}
				TreeContent::Inline(_) | TreeContent::Leaf => false,
			}
		});

		if !fits {
			return Ok(None);
		}
	}

	let parent = parent_of(doc, list);
	let index = doc.index_in_parent(list).unwrap_or_default();

	state.change(EditKind::Structure, |tr| {
		tr.remove_blocks(parent, index, 1)?;
		tr.insert_blocks(parent, index, vec![tree])?;
		tr.set_selection(selection);
		Ok(true)
	})
}

/// Wrap the selected sibling blocks in a new `target` list, one item each,
/// joining it with neighbouring lists of the same type. Text blocks the item
/// does not accept become its default text block. Unlike [`toggle_list`],
/// this nests a new list when the selection is already inside one, which is
/// what typing `- ` in a list item means.
pub fn wrap_in_list(state: &mut EditorState, target: ListType) -> CommandResult {
	let selection = state.selection();
	let doc = state.doc();
	let Some((first, last)) = selection_ends(doc, selection) else {
		return Ok(None);
	};
	let Some((parent, range)) = position::sibling_range(doc, first, last) else {
		return Ok(None);
	};
	let item_text = structure::text_type_in(doc.schema(), Some(target.item));
	let children = doc.children(parent)[range.clone()].to_vec();

	let mut convert = Vec::new();

	for &child in &children {
		let Some(kind) = kind_of(doc, child) else {
			return Ok(None);
		};

		if doc.schema().allows_child(Some(target.item), kind) {
			continue;
		}

		match item_text {
			Some(text) if doc.inline(child).is_some() && structure::allows(doc, parent, text) => {
				convert.push((child, text));
			}
			_ => return Ok(None),
		}
	}

	if !structure::allows(doc, parent, target.list) {
		return Ok(None);
	}

	state.change(EditKind::Structure, |tr| {
		for (child, text) in convert {
			let attrs = structure::default_attrs(tr.doc(), text);
			tr.set_block(child, text, attrs)?;
		}

		let wrapper = structure::fresh_block(tr, target.list)?;
		let wrapper_id = wrapper.id;
		tr.insert_blocks(parent, range.start, vec![wrapper])?;

		for index in 0..range.len() {
			let entry = structure::fresh_block(tr, target.item)?;
			let entry_id = entry.id;
			tr.insert_blocks(Some(wrapper_id), index, vec![entry])?;
			tr.move_blocks((parent, range.start + 1, 1), (Some(entry_id), 0))?;
		}

		join_adjacent(tr, wrapper_id, range.start)?;
		tr.set_selection(selection);
		Ok(true)
	})
}

/// Merge `list` with neighbouring lists of the same type.
/// `index` is where `list` sits among its siblings.
fn join_adjacent(
	tr: &mut EditorTransaction<'_>,
	list: BlockId,
	index: usize,
) -> Result<(), EditorError> {
	let doc = tr.doc();
	let parent = parent_of(doc, list);
	let kind = kind_of(doc, list);
	let siblings = doc.children(parent);
	let previous = index.checked_sub(1).map(|previous| siblings[previous]);
	let next = siblings.get(index + 1).copied();
	let mut current = list;

	if let Some(previous) = previous.filter(|&id| kind_of(doc, id) == kind) {
		absorb(tr, previous, current)?;
		current = previous;
	}

	if let Some(next) = next.filter(|&id| kind_of(tr.doc(), id) == kind) {
		absorb(tr, current, next)?;
	}

	Ok(())
}

/// Move every child of `from` to the end of `into`, then remove `from`.
fn absorb(tr: &mut EditorTransaction<'_>, into: BlockId, from: BlockId) -> Result<(), EditorError> {
	let count = tr.doc().children(Some(from)).len();
	let end = tr.doc().children(Some(into)).len();
	tr.move_blocks((Some(from), 0, count), (Some(into), end))?;
	structure::remove(tr, from)
}
