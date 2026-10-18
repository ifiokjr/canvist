//! Remote changes: yrs events become model steps.
//!
//! Records from one yrs transaction are translated in three passes:
//!
//! 1. Text deltas and attribute changes become inline steps and `SetBlock`.
//!    They never touch child lists, so array deltas stay valid.
//! 2. Array deltas become block removals, moves, and insertions (see
//!    [`plan`]).
//! 3. Changes that alter which blocks are visible (a type that is no longer
//!    allowed, a replaced `text` or `children`, a hidden block that became
//!    valid) are resolved last, reading the final shared state.
//!
//! Every pass reads only the shared types the transaction changed, so the
//! cost follows the size of the change, not the size of the document.

mod plan;
pub(crate) mod record;

use std::mem::discriminant;
use std::ops::Range;
use std::sync::Arc;

use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::Commit;
use canvist_model::Content;
use canvist_model::InlineText;
use canvist_model::Schema;
use canvist_model::Step;
use canvist_model::StepError;
use canvist_model::Transaction;
use rustc_hash::FxHashSet;
use yrs::Array;
use yrs::ArrayRef;
use yrs::ID;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;

use crate::CollabError;
use crate::claims::Owners;
use crate::keys;
use crate::local::map_ref;
use crate::registry::Owner;
use crate::registry::Registration;
use crate::registry::Registry;
use crate::registry::branch_id;
use crate::remote::plan::Incoming;
use crate::remote::plan::Plan;
use crate::remote::plan::RemoteClaims;
use crate::remote::record::Record;
use crate::remote::record::TextOp;
use crate::text;
use crate::text::Cursor;
use crate::tree;
use crate::tree::Parent;
use crate::tree::Reader;
use crate::values;
use crate::values::Format;

/// A block map the binding must delete to settle duplicate block ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Repair {
	/// The parent whose array holds the map, `None` for the root.
	pub(crate) parent: Option<BlockId>,
	/// The map's item id.
	pub(crate) map: ID,
}

/// Visibility changes resolved after the structural pass.
#[derive(Debug, Default)]
struct Later {
	/// Blocks whose type or content no longer fits where they are.
	hide: Vec<BlockId>,
	/// Blocks whose id, text, or children were replaced.
	rebuild: Vec<BlockId>,
	/// Containers whose type changed, so their children need checking.
	retype: Vec<BlockId>,
	/// Hidden block maps that changed and may now be valid.
	unhide: Vec<(ID, Option<BlockId>)>,
}

/// Translates one yrs transaction's records into one model commit.
pub(crate) struct Remote<'a, 'doc, T: ReadTxn> {
	tx: Transaction<'doc>,
	txn: &'a T,
	root: &'a ArrayRef,
	schema: &'a Schema,
	registry: &'a mut Registry,
	repairs: Vec<Repair>,
	later: Later,
}

impl<'a, 'doc, T: ReadTxn> Remote<'a, 'doc, T> {
	/// Start translating into `tx`, reading the shared state through `txn`.
	pub(crate) fn new(
		tx: Transaction<'doc>,
		txn: &'a T,
		(root, schema): (&'a ArrayRef, &'a Schema),
		registry: &'a mut Registry,
	) -> Self {
		Self {
			tx,
			txn,
			root,
			schema,
			registry,
			repairs: Vec::new(),
			later: Later::default(),
		}
	}

	/// Translate `records` and commit, or roll back and fail. Also returns
	/// the duplicate block maps to delete.
	pub(crate) fn apply(
		mut self,
		records: Vec<Record>,
	) -> Result<(Commit, Vec<Repair>), CollabError> {
		let result = self.translate(records);
		self.finish(result)
	}

	/// Show hidden children that the new types of `containers` allow, after
	/// a local commit retyped them.
	pub(crate) fn reveal(
		mut self,
		containers: &[BlockId],
	) -> Result<(Commit, Vec<Repair>), CollabError> {
		let result = containers.iter().try_for_each(|&container| {
			for map in self.registry.hidden_in(Some(container)) {
				self.unhide(map, Some(container))?;
			}

			Ok(())
		});
		self.finish(result)
	}

	fn finish(self, result: Result<(), CollabError>) -> Result<(Commit, Vec<Repair>), CollabError> {
		match result {
			Ok(()) => Ok((self.tx.commit(), self.repairs)),
			Err(error) => {
				self.tx.rollback()?;
				Err(error)
			}
		}
	}

	fn translate(&mut self, records: Vec<Record>) -> Result<(), CollabError> {
		let mut plan = Plan::default();

		for record in records {
			match record {
				Record::Text { text, ops } => self.text(&text, ops)?,
				Record::Map { map, keys } => self.map(&map, &keys)?,
				Record::Array {
					array,
					ops,
					inserted,
					removed,
				} => self.plan_array(array, &ops, (&inserted, &removed), &mut plan)?,
			}
		}

		self.execute(plan)?;
		self.resolve_later()
	}

	/// Apply a `Y.Text` delta. Positions are UTF-16 and advance through the
	/// text as the delta edits it, so a cursor tracks both units while each
	/// operation becomes a step at the cursor's UTF-8 offset.
	fn text(&mut self, text: &ID, ops: Vec<TextOp>) -> Result<(), CollabError> {
		let Some(Owner::Text(block)) = self.registry.owner(text) else {
			return Ok(());
		};
		let mut cursor = Cursor::default();

		for op in ops {
			self.text_op(block, &mut cursor, op)?;
		}

		Ok(())
	}

	fn text_op(
		&mut self,
		block: BlockId,
		cursor: &mut Cursor,
		op: TextOp,
	) -> Result<(), CollabError> {
		match op {
			TextOp::Retain(units) => self.advance(block, cursor, units),
			TextOp::Format(units, format) => {
				let start = cursor.utf8;
				self.advance(block, cursor, units)?;
				self.reformat(block, start..cursor.utf8, &format)
			}
			TextOp::Delete(units) => {
				let mut end = *cursor;
				self.advance(block, &mut end, units)?;
				let range = cursor.utf8..end.utf8;
				self.tx.step(Step::RemoveInline { block, range })?;
				Ok(())
			}
			TextOp::Insert(content) => self.insert_inline(block, cursor, content),
		}
	}

	/// Insert `content` at `cursor` and move the cursor past it.
	fn insert_inline(
		&mut self,
		block: BlockId,
		cursor: &mut Cursor,
		content: InlineText,
	) -> Result<(), CollabError> {
		if content.is_empty() {
			return Ok(());
		}

		let (bytes, units) = (content.len(), text::utf16_len(content.as_str()));
		let offset = cursor.utf8;
		self.tx.step(Step::InsertInline {
			block,
			offset,
			content,
		})?;
		cursor.utf8 += bytes;
		cursor.utf16 += units;
		Ok(())
	}

	fn advance(&self, block: BlockId, cursor: &mut Cursor, units: u32) -> Result<(), CollabError> {
		let offset = cursor.utf16 + units;
		let text = self
			.tx
			.doc()
			.inline(block)
			.ok_or(CollabError::NotShared(block))?;
		cursor
			.advance(text.as_str(), units)
			.ok_or(CollabError::Offset { block, offset })
	}

	fn reformat(
		&mut self,
		block: BlockId,
		range: Range<u32>,
		format: &Format,
	) -> Result<(), CollabError> {
		let text = self
			.tx
			.doc()
			.inline(block)
			.ok_or(CollabError::NotShared(block))?;
		let runs = text.mark_runs(range.clone()).map_err(StepError::from)?;
		let formatted: Vec<_> = runs
			.iter()
			.map(|(len, marks)| (*len, values::apply_format(self.schema, marks, format)))
			.collect();

		if formatted != runs {
			self.tx.step(Step::RestoreMarks {
				block,
				offset: range.start,
				runs: formatted,
			})?;
		}

		Ok(())
	}

	/// Mirror changed keys of a block map or its attributes. Replacing a
	/// block's id or content map means re-reading it; other changes retype.
	fn map(&mut self, map: &ID, changed: &[Arc<str>]) -> Result<(), CollabError> {
		match self.registry.owner(map) {
			Some(Owner::Block(block)) if replaces_content(changed) => {
				self.later.rebuild.push(block);
				Ok(())
			}
			Some(Owner::Block(block) | Owner::Attrs(block)) => self.retype(block),
			Some(Owner::Hidden(parent)) => {
				self.later.unhide.push((*map, parent));
				Ok(())
			}
			_ => Ok(()),
		}
	}

	/// Mirror a change to a block's type or attributes.
	fn retype(&mut self, block: BlockId) -> Result<(), CollabError> {
		let map = self.map_of(block)?;
		let Some((_, kind)) = tree::identity(&map, self.txn, self.schema) else {
			self.later.hide.push(block);

			return Ok(());
		};
		let (attrs, attrs_id) = tree::read_attrs(&map, self.txn);
		self.registry.set_attrs(block, attrs_id);

		let current = self
			.tx
			.doc()
			.block(block)
			.ok_or(CollabError::NotShared(block))?;

		if current.kind() == kind && *current.attrs() == attrs {
			return Ok(());
		}

		if !self.fits(current, kind) {
			self.later.hide.push(block);
		} else if current.kind() != kind && matches!(current.content(), Content::Blocks(_)) {
			self.later.retype.push(block);
		} else {
			self.tx.step(Step::SetBlock { block, kind, attrs })?;
		}

		Ok(())
	}

	/// Whether `block` may take type `kind` where it is.
	fn fits(&self, block: &Block, kind: BlockTypeId) -> bool {
		let same_content = discriminant(&self.schema.block(block.kind()).content)
			== discriminant(&self.schema.block(kind).content);
		let parent_kind = self.parent_kind(block.parent());
		same_content && self.schema.allows_child(parent_kind, kind)
	}

	/// Resolve visibility changes in a fixed order: hiding first, so a
	/// container's children are checked against its final type, then
	/// rebuilding, retyping containers, and finally showing hidden maps.
	fn resolve_later(&mut self) -> Result<(), CollabError> {
		let later = std::mem::take(&mut self.later);

		for block in later.hide {
			self.if_shown(block, Self::hide)?;
		}

		for block in later.rebuild {
			self.if_shown(block, Self::rebuild)?;
		}

		for block in later.retype {
			self.if_shown(block, Self::retype_container)?;
		}

		for (map, parent) in later.unhide {
			self.unhide(map, parent)?;
		}

		Ok(())
	}

	/// Run `change` on `block` unless an earlier change removed it.
	fn if_shown(
		&mut self,
		block: BlockId,
		change: fn(&mut Self, BlockId) -> Result<(), CollabError>,
	) -> Result<(), CollabError> {
		if self.tx.doc().contains(block) {
			change(self, block)
		} else {
			Ok(())
		}
	}

	/// Stop showing `block`, keeping its map as hidden.
	fn hide(&mut self, block: BlockId) -> Result<(), CollabError> {
		let found = self
			.registry
			.block(block)
			.ok_or(CollabError::NotShared(block))?;
		self.remove_block(block)?;
		self.registry.insert_hidden(found.map, found.parent);
		Ok(())
	}

	/// Re-read `block` from scratch after its id or content was replaced.
	fn rebuild(&mut self, block: BlockId) -> Result<(), CollabError> {
		let found = self
			.registry
			.block(block)
			.ok_or(CollabError::NotShared(block))?;
		self.hide(block)?;
		self.unhide(found.map, found.parent)
	}

	/// Apply a container's new type, hiding children it no longer allows and
	/// showing hidden children it now allows.
	fn retype_container(&mut self, block: BlockId) -> Result<(), CollabError> {
		let map = self.map_of(block)?;
		let Some((_, kind)) = tree::identity(&map, self.txn, self.schema) else {
			return self.hide(block);
		};
		let (attrs, attrs_id) = tree::read_attrs(&map, self.txn);
		self.registry.set_attrs(block, attrs_id);
		let children = self.tx.doc().children(Some(block)).to_vec();

		for child in children {
			let allowed = self
				.tx
				.doc()
				.block(child)
				.is_some_and(|found| self.schema.allows_child(Some(kind), found.kind()));

			if !allowed {
				self.hide(child)?;
			}
		}

		self.tx.step(Step::SetBlock { block, kind, attrs })?;

		for map in self.registry.hidden_in(Some(block)) {
			self.unhide(map, Some(block))?;
		}

		Ok(())
	}

	/// Show the hidden block map `map` inside `parent` if it is now valid.
	fn unhide(&mut self, map: ID, parent: Option<BlockId>) -> Result<(), CollabError> {
		let still_hidden = self.registry.owner(&map) == Some(Owner::Hidden(parent));
		let parent_shown = parent.is_none_or(|parent| self.tx.doc().contains(parent));

		if !still_hidden || !parent_shown {
			return Ok(());
		}

		let Some(shared) = map_ref(self.txn, &map) else {
			self.registry.remove_hidden(&map);
			return Ok(());
		};
		let (tree, registration, evicted) = self.read_hidden(&shared, parent);
		self.repair_losers(&registration);

		let Some(tree) = tree else {
			return Ok(());
		};

		for block in evicted {
			self.evict(block)?;
		}

		let index = self.shown_before(&self.array(parent)?, &map);
		self.registry.remove_hidden(&map);
		self.tx.step(Step::InsertBlocks {
			parent,
			index,
			blocks: vec![tree],
		})?;
		self.registry.register(registration);
		Ok(())
	}

	/// Read a hidden map as if it were newly inserted into `parent`,
	/// returning its tree if it is now valid and any blocks it evicts.
	fn read_hidden(
		&self,
		shared: &MapRef,
		parent: Option<BlockId>,
	) -> (Option<BlockTree>, Registration, Vec<BlockId>) {
		let mut incoming = Owners::default();
		incoming.scan(shared, self.txn);
		let none = FxHashSet::default();
		let mut claims = RemoteClaims::new(
			self.tx.doc(),
			self.registry,
			(Incoming::Scanned(&incoming), &none),
		);
		claims.target = parent;
		let mut registration = Registration::default();
		let mut reader = Reader {
			txn: self.txn,
			schema: self.schema,
			claims: &mut claims,
		};
		let found = Parent {
			id: parent,
			kind: self.parent_kind(parent),
		};
		let tree = reader.block(shared, found, &mut registration);

		(tree, registration, claims.evicted)
	}

	/// How many shown blocks precede the map `map` in `array`.
	fn shown_before(&self, array: &ArrayRef, map: &ID) -> usize {
		array
			.iter(self.txn)
			.take_while(
				|item| !matches!(item, Out::YMap(found) if branch_id(found).as_ref() == Some(map)),
			)
			.filter(|item| {
				matches!(item, Out::YMap(found)
					if branch_id(found).is_some_and(|id| self.registry.block_of_map(&id).is_some()))
			})
			.count()
	}

	/// Remove a block that lost its id to another map, and delete its map.
	fn evict(&mut self, block: BlockId) -> Result<(), CollabError> {
		let Some(found) = self.registry.block(block) else {
			return Ok(());
		};
		self.repairs.push(Repair {
			parent: found.parent,
			map: found.map,
		});
		self.hide(block)
	}

	fn repair_losers(&mut self, registration: &Registration) {
		self.repairs.extend(
			registration
				.losers
				.iter()
				.map(|&(map, parent)| Repair { parent, map }),
		);
	}

	/// Remove `block` and its descendants from the model and the registry.
	fn remove_block(&mut self, block: BlockId) -> Result<(), CollabError> {
		let doc = self.tx.doc();
		let found = doc.block(block).ok_or(CollabError::NotShared(block))?;
		let parent = found.parent();
		let index = doc
			.index_in_parent(block)
			.ok_or(CollabError::NotShared(block))?;
		let ids = descendants(doc, block);
		self.tx.step(Step::RemoveBlocks {
			parent,
			index,
			count: 1,
		})?;
		self.registry.remove(&ids);
		Ok(())
	}

	fn map_of(&self, block: BlockId) -> Result<MapRef, CollabError> {
		let found = self
			.registry
			.block(block)
			.ok_or(CollabError::NotShared(block))?;
		map_ref(self.txn, &found.map).ok_or(CollabError::NotShared(block))
	}

	/// The array holding the children of `parent`.
	fn array(&self, parent: Option<BlockId>) -> Result<ArrayRef, CollabError> {
		let Some(parent) = parent else {
			return Ok(self.root.clone());
		};

		tree::children_of(&self.map_of(parent)?, self.txn).ok_or(CollabError::NotShared(parent))
	}

	fn parent_kind(&self, parent: Option<BlockId>) -> Option<BlockTypeId> {
		parent
			.and_then(|parent| self.tx.doc().block(parent))
			.map(Block::kind)
	}
}

/// Whether changing these keys of a block map replaces its identity or
/// content, rather than its type or attributes.
fn replaces_content(changed: &[Arc<str>]) -> bool {
	changed
		.iter()
		.any(|key| [keys::ID, keys::TEXT, keys::CHILDREN].contains(&&**key))
}

/// `block` and every block inside it.
pub(crate) fn descendants(doc: &canvist_model::Document, block: BlockId) -> FxHashSet<BlockId> {
	let mut ids = FxHashSet::default();
	let mut stack = vec![block];

	while let Some(id) = stack.pop() {
		ids.insert(id);
		stack.extend_from_slice(doc.children(Some(id)));
	}

	ids
}
