//! Reading and writing block maps.

use std::sync::Arc;

use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::ContentKind;
use canvist_model::Schema;
use canvist_model::TreeContent;
use yrs::Any;
use yrs::Array;
use yrs::ArrayPrelim;
use yrs::ArrayRef;
use yrs::In;
use yrs::Map;
use yrs::MapPrelim;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;
use yrs::Text;
use yrs::TextPrelim;
use yrs::TextRef;
use yrs::TransactionMut;

use crate::CollabError;
use crate::claims::Claim;
use crate::keys;
use crate::registry::BlockRef;
use crate::registry::Registration;
use crate::registry::branch_id;
use crate::text;
use crate::text::Chunks;
use crate::values;

/// Where a block sits: its parent's id and type (`None` for the root).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Parent {
	/// The parent block, or `None` for the root.
	pub(crate) id: Option<BlockId>,
	/// The parent's type, or `None` for the root.
	pub(crate) kind: Option<BlockTypeId>,
}

impl Parent {
	/// The document root.
	pub(crate) const ROOT: Self = Self {
		id: None,
		kind: None,
	};
}

/// Reads block maps into model trees, deciding which blocks the model shows.
///
/// A block map is shown when its id parses, its type is in the schema and
/// allowed in its parent, and its content matches the type. Anything else is
/// recorded as hidden so the binding can keep indices aligned and show it
/// later if it becomes valid. When several maps carry the same id, `claims`
/// decides which one shows; the others are recorded as losers to delete.
pub(crate) struct Reader<'a, T: ReadTxn> {
	pub(crate) txn: &'a T,
	pub(crate) schema: &'a Schema,
	pub(crate) claims: &'a mut dyn Claim,
}

/// The content a block map holds, matching its type.
enum Shape {
	Text(TextRef),
	Children(ArrayRef),
	Leaf,
}

impl<T: ReadTxn> Reader<'_, T> {
	/// Read the block map `map` inside `parent`, or `None` if it is hidden.
	pub(crate) fn block(
		&mut self,
		map: &MapRef,
		parent: Parent,
		registration: &mut Registration,
	) -> Option<BlockTree> {
		let map_id = branch_id(map)?;
		let (id, kind, shape) = self.header(map, parent)?;

		if !self.claims.claim(id, &map_id) {
			registration.losers.push((map_id, parent.id));

			return None;
		}

		let (attrs, attrs_id) = read_attrs(map, self.txn);
		let mut block = BlockRef {
			map: map_id,
			parent: parent.id,
			text: None,
			attrs: attrs_id,
			children: None,
		};

		let content = match shape {
			Shape::Text(text) => {
				block.text = branch_id(&text);
				TreeContent::Inline(Chunks::read(&text, self.txn).to_inline(self.schema))
			}
			Shape::Children(children) => {
				block.children = branch_id(&children);
				let parent = Parent {
					id: Some(id),
					kind: Some(kind),
				};
				TreeContent::Blocks(self.children(&children, parent, registration))
			}
			Shape::Leaf => TreeContent::Leaf,
		};

		registration.blocks.push((id, block));
		Some(BlockTree {
			id,
			kind,
			attrs,
			content,
		})
	}

	/// Read every shown block in `array`, the children of `parent`.
	pub(crate) fn children(
		&mut self,
		array: &ArrayRef,
		parent: Parent,
		registration: &mut Registration,
	) -> Vec<BlockTree> {
		let mut trees = Vec::new();
		let mut unmirrored = 0;

		for item in array.iter(self.txn) {
			let Out::YMap(map) = item else {
				unmirrored += 1;
				continue;
			};

			if let Some(tree) = self.block(&map, parent, registration) {
				trees.push(tree);
				continue;
			}

			registration
				.hidden
				.extend(branch_id(&map).map(|map| (map, parent.id)));
			unmirrored += 1;
		}

		if unmirrored > 0 {
			registration.unmirrored.push((parent.id, unmirrored));
		}

		trees
	}

	/// The id, type, and content of a valid block map inside `parent`.
	fn header(&self, map: &MapRef, parent: Parent) -> Option<(BlockId, BlockTypeId, Shape)> {
		let (id, kind) = identity(map, self.txn, self.schema)?;

		if !self.schema.allows_child(parent.kind, kind) {
			return None;
		}

		let shape = match &self.schema.block(kind).content {
			ContentKind::Inline => Shape::Text(text_of(map, self.txn)?),
			ContentKind::Blocks(_) => Shape::Children(children_of(map, self.txn)?),
			ContentKind::Leaf => Shape::Leaf,
		};

		Some((id, kind, shape))
	}
}

/// The parsed id and type of a block map.
pub(crate) fn identity<T: ReadTxn>(
	map: &MapRef,
	txn: &T,
	schema: &Schema,
) -> Option<(BlockId, BlockTypeId)> {
	let id = string(map, txn, keys::ID)?.parse::<BlockId>().ok()?;
	let kind = schema.block_type(&string(map, txn, keys::TYPE)?)?;
	Some((id, kind))
}

/// The attributes of a block map and the item id of their `Y.Map`.
pub(crate) fn read_attrs<T: ReadTxn>(map: &MapRef, txn: &T) -> (Attrs, Option<yrs::ID>) {
	let Some(shared) = attrs_of(map, txn) else {
		return (Attrs::new(), None);
	};
	let entries: Vec<(Arc<str>, Any)> = shared
		.iter(txn)
		.filter_map(|(key, value)| {
			match value {
				Out::Any(any) => Some((Arc::from(key), any)),
				_ => None,
			}
		})
		.collect();
	let attrs = values::attrs_from(entries.iter().map(|(key, any)| (&**key, any)));

	(attrs, branch_id(&shared))
}

pub(crate) fn string<T: ReadTxn>(map: &MapRef, txn: &T, key: &str) -> Option<Arc<str>> {
	match map.get(txn, key)? {
		Out::Any(Any::String(value)) => Some(value),
		_ => None,
	}
}

/// The `Y.Text` of a block map.
pub(crate) fn text_of<T: ReadTxn>(map: &MapRef, txn: &T) -> Option<TextRef> {
	match map.get(txn, keys::TEXT)? {
		Out::YText(text) => Some(text),
		_ => None,
	}
}

/// The child `Y.Array` of a block map.
pub(crate) fn children_of<T: ReadTxn>(map: &MapRef, txn: &T) -> Option<ArrayRef> {
	match map.get(txn, keys::CHILDREN)? {
		Out::YArray(children) => Some(children),
		_ => None,
	}
}

/// The attribute `Y.Map` of a block map.
pub(crate) fn attrs_of<T: ReadTxn>(map: &MapRef, txn: &T) -> Option<MapRef> {
	match map.get(txn, keys::ATTRS)? {
		Out::YMap(attrs) => Some(attrs),
		_ => None,
	}
}

/// Write `tree` as a new block map at `index` of `array`, the children of
/// `parent`.
pub(crate) fn write(
	txn: &mut TransactionMut<'_>,
	schema: &Schema,
	target: (&ArrayRef, u32, Option<BlockId>),
	tree: &BlockTree,
	registration: &mut Registration,
) -> Result<(), CollabError> {
	let (array, index, parent) = target;
	let map: MapRef = array.insert(txn, index, MapPrelim::default());
	map.insert(txn, keys::ID, tree.id.to_string());
	map.insert(txn, keys::TYPE, &*schema.block(tree.kind).name);
	let attrs: MapRef = map.insert(txn, keys::ATTRS, attrs_prelim(&tree.attrs));
	let mut block = BlockRef {
		map: nested_id(&map)?,
		parent,
		text: None,
		attrs: branch_id(&attrs),
		children: None,
	};

	match &tree.content {
		TreeContent::Inline(inline) => {
			let text: TextRef = map.insert(txn, keys::TEXT, TextPrelim::new(""));
			text.apply_delta(txn, text::inline_delta(schema, inline));
			block.text = branch_id(&text);
		}
		TreeContent::Blocks(children) => {
			let array: ArrayRef = map.insert(txn, keys::CHILDREN, ArrayPrelim::default());
			block.children = branch_id(&array);

			for (offset, child) in (0_u32..).zip(children) {
				write(
					txn,
					schema,
					(&array, offset, Some(tree.id)),
					child,
					registration,
				)?;
			}
		}
		TreeContent::Leaf => {}
	}

	registration.blocks.push((tree.id, block));
	Ok(())
}

/// The item id of a shared type the binding created inside another one.
pub(crate) fn nested_id(branch: &impl AsRef<yrs::branch::Branch>) -> Result<yrs::ID, CollabError> {
	branch_id(branch).ok_or(CollabError::RootType)
}

/// Attributes as a `Y.Map` prelim.
pub(crate) fn attrs_prelim(attrs: &Attrs) -> MapPrelim {
	attrs
		.iter()
		.map(|(key, value)| (key, In::Any(values::to_any(value))))
		.collect()
}
