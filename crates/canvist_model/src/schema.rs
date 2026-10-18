//! Schemas describe which blocks, marks, and inline embeds a document allows.
//!
//! Extensions contribute [`BlockSpec`]s, [`MarkSpec`]s, and [`EmbedSpec`]s to a
//! [`SchemaBuilder`]; [`SchemaBuilder::build`] resolves every name into a
//! compact numeric id so that documents store `u16` type ids instead of
//! strings. Only the extensions an app registers end up in its schema, which
//! is how canvist keeps "pay for what you use".

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::Attrs;

macro_rules! type_id {
	($(#[$meta:meta])* $name:ident) => {
		$(#[$meta])*
		#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
		pub struct $name(u16);

		impl $name {
			/// The index of this type within its schema.
			#[must_use]
			pub const fn index(self) -> usize {
				self.0 as usize
			}
		}
	};
}

type_id!(
	/// Identifies a block type within one [`Schema`].
	BlockTypeId
);
type_id!(
	/// Identifies a mark type within one [`Schema`].
	MarkTypeId
);
type_id!(
	/// Identifies an inline embed type within one [`Schema`].
	EmbedTypeId
);

/// What a block type may contain, as declared by an extension.
#[derive(Clone, Debug, PartialEq)]
pub enum ContentSpec {
	/// Inline text with marks and embeds, e.g. a paragraph or heading.
	Inline,
	/// Child blocks whose type name or group appears in the list, e.g. a list
	/// containing `list_item`s or a blockquote containing the `block` group.
	Blocks(Vec<Arc<str>>),
	/// No content at all, e.g. a divider or an image.
	Leaf,
}

/// Declares a block type.
#[derive(Clone, Debug)]
pub struct BlockSpec {
	/// Unique name, e.g. `paragraph`.
	pub name: Arc<str>,
	/// What the block may contain.
	pub content: ContentSpec,
	/// Groups this block belongs to, e.g. `block`, usable in content rules.
	pub groups: Vec<Arc<str>>,
	/// Default attributes for newly created blocks.
	pub attrs: Attrs,
}

impl BlockSpec {
	/// A block holding inline text, in the `block` group.
	#[must_use]
	pub fn text(name: &str) -> Self {
		Self::new(name, ContentSpec::Inline)
	}

	/// A block holding child blocks of the given types or groups.
	#[must_use]
	pub fn container(name: &str, children: &[&str]) -> Self {
		let children = children.iter().map(|&child| child.into()).collect();
		Self::new(name, ContentSpec::Blocks(children))
	}

	/// A block with no content.
	#[must_use]
	pub fn leaf(name: &str) -> Self {
		Self::new(name, ContentSpec::Leaf)
	}

	/// Replace the default attributes.
	#[must_use]
	pub fn attrs(mut self, attrs: Attrs) -> Self {
		self.attrs = attrs;
		self
	}

	/// Replace the groups (the default is `["block"]`).
	#[must_use]
	pub fn groups(mut self, groups: &[&str]) -> Self {
		self.groups = groups.iter().map(|&group| group.into()).collect();
		self
	}

	fn new(name: &str, content: ContentSpec) -> Self {
		Self {
			name: name.into(),
			content,
			groups: vec!["block".into()],
			attrs: Attrs::new(),
		}
	}
}

/// Declares a mark type such as bold or link.
#[derive(Clone, Debug)]
pub struct MarkSpec {
	/// Unique name, e.g. `bold`.
	pub name: Arc<str>,
	/// Whether text typed at the end of the mark extends it. Links are
	/// usually not inclusive; bold usually is.
	pub inclusive: bool,
	/// Other mark types that cannot coexist with this one. A mark always
	/// excludes other instances of its own type.
	pub excludes: Vec<Arc<str>>,
	/// Default attributes.
	pub attrs: Attrs,
}

impl MarkSpec {
	/// An inclusive mark with no exclusions.
	#[must_use]
	pub fn new(name: &str) -> Self {
		Self {
			name: name.into(),
			inclusive: true,
			excludes: Vec::new(),
			attrs: Attrs::new(),
		}
	}

	/// Set whether typing at the mark's end extends it.
	#[must_use]
	pub fn inclusive(mut self, inclusive: bool) -> Self {
		self.inclusive = inclusive;
		self
	}

	/// Declare mark types this mark cannot coexist with.
	#[must_use]
	pub fn excludes(mut self, names: &[&str]) -> Self {
		self.excludes = names.iter().map(|&name| name.into()).collect();
		self
	}

	/// Replace the default attributes.
	#[must_use]
	pub fn attrs(mut self, attrs: Attrs) -> Self {
		self.attrs = attrs;
		self
	}
}

/// Declares an inline embed: an atomic inline object such as a mention, an
/// inline image, or a hard line break.
#[derive(Clone, Debug)]
pub struct EmbedSpec {
	/// Unique name, e.g. `mention`.
	pub name: Arc<str>,
	/// Default attributes.
	pub attrs: Attrs,
}

impl EmbedSpec {
	/// An embed with no default attributes.
	#[must_use]
	pub fn new(name: &str) -> Self {
		Self {
			name: name.into(),
			attrs: Attrs::new(),
		}
	}
}

/// Resolved content rule of a [`BlockType`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentKind {
	/// Inline text.
	Inline,
	/// Child blocks of the listed types.
	Blocks(Vec<BlockTypeId>),
	/// No content.
	Leaf,
}

/// A resolved block type.
#[derive(Clone, Debug)]
pub struct BlockType {
	/// The type's id.
	pub id: BlockTypeId,
	/// The type's name.
	pub name: Arc<str>,
	/// What the block may contain.
	pub content: ContentKind,
	/// Default attributes.
	pub attrs: Attrs,
}

/// A resolved mark type.
#[derive(Clone, Debug)]
pub struct MarkType {
	/// The type's id. Marks in a set are ordered by id.
	pub id: MarkTypeId,
	/// The type's name.
	pub name: Arc<str>,
	/// Whether typing at the end of the mark extends it.
	pub inclusive: bool,
	/// Mark types removed when this mark is added, always including itself.
	pub excludes: Vec<MarkTypeId>,
	/// Default attributes.
	pub attrs: Attrs,
}

/// A resolved inline embed type.
#[derive(Clone, Debug)]
pub struct EmbedType {
	/// The type's id.
	pub id: EmbedTypeId,
	/// The type's name.
	pub name: Arc<str>,
	/// Default attributes.
	pub attrs: Attrs,
}

/// Errors raised while building a [`Schema`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
	/// Two specs of the same kind share a name.
	#[error("duplicate {kind} type `{name}`")]
	Duplicate {
		/// `block`, `mark`, or `embed`.
		kind: &'static str,
		/// The duplicated name.
		name: Arc<str>,
	},
	/// A content rule or exclusion names something that does not exist.
	#[error("`{owner}` refers to unknown {kind} or group `{name}`")]
	Unknown {
		/// The spec containing the reference.
		owner: Arc<str>,
		/// `block` or `mark`.
		kind: &'static str,
		/// The unknown name.
		name: Arc<str>,
	},
	/// The root allows no inline-content block, so text cannot be typed.
	#[error("the root content must allow at least one text block")]
	NoTextBlock,
	/// More than 65 536 types of one kind were registered.
	#[error("too many {0} types")]
	TooMany(&'static str),
}

/// Collects specs and resolves them into a [`Schema`].
#[derive(Clone, Debug, Default)]
pub struct SchemaBuilder {
	blocks: Vec<BlockSpec>,
	marks: Vec<MarkSpec>,
	embeds: Vec<EmbedSpec>,
	root: Vec<Arc<str>>,
}

impl SchemaBuilder {
	/// An empty builder whose root accepts the `block` group.
	#[must_use]
	pub fn new() -> Self {
		Self {
			root: vec!["block".into()],
			..Self::default()
		}
	}

	/// Register a block type.
	#[must_use]
	pub fn block(mut self, spec: BlockSpec) -> Self {
		self.blocks.push(spec);
		self
	}

	/// Register a mark type. Registration order defines mark ordering.
	#[must_use]
	pub fn mark(mut self, spec: MarkSpec) -> Self {
		self.marks.push(spec);
		self
	}

	/// Register an inline embed type.
	#[must_use]
	pub fn embed(mut self, spec: EmbedSpec) -> Self {
		self.embeds.push(spec);
		self
	}

	/// Set the block types or groups allowed at the document root.
	#[must_use]
	pub fn root(mut self, children: &[&str]) -> Self {
		self.root = children.iter().map(|&child| child.into()).collect();
		self
	}

	/// Resolve every name and validate the schema.
	pub fn build(self) -> Result<Schema, SchemaError> {
		let block_names = index_names("block", self.blocks.iter().map(|spec| &spec.name))?;
		let mark_names = index_names("mark", self.marks.iter().map(|spec| &spec.name))?;
		let embed_names = index_names("embed", self.embeds.iter().map(|spec| &spec.name))?;
		let resolver = ChildResolver::new(&self.blocks, &block_names);

		let blocks = self
			.blocks
			.iter()
			.enumerate()
			.map(|(index, spec)| resolve_block(index, spec, &resolver))
			.collect::<Result<Vec<_>, _>>()?;
		let marks = self
			.marks
			.iter()
			.enumerate()
			.map(|(index, spec)| resolve_mark(index, spec, &mark_names))
			.collect::<Result<Vec<_>, _>>()?;
		let embeds = self
			.embeds
			.into_iter()
			.enumerate()
			.map(|(index, spec)| resolve_embed(index, spec))
			.collect();
		let root = resolver.resolve(&"doc".into(), &self.root)?;

		let default_text_block = root
			.iter()
			.copied()
			.find(|id| blocks[id.index()].content == ContentKind::Inline)
			.ok_or(SchemaError::NoTextBlock)?;

		Ok(Schema {
			blocks,
			marks,
			embeds,
			root,
			default_text_block,
			block_names,
			mark_names,
			embed_names,
		})
	}
}

fn index_names<'a>(
	kind: &'static str,
	names: impl Iterator<Item = &'a Arc<str>>,
) -> Result<FxHashMap<Arc<str>, u16>, SchemaError> {
	let mut index = FxHashMap::default();

	for (position, name) in names.enumerate() {
		let id = u16::try_from(position).map_err(|_| SchemaError::TooMany(kind))?;

		if index.insert(name.clone(), id).is_some() {
			return Err(SchemaError::Duplicate {
				kind,
				name: name.clone(),
			});
		}
	}

	Ok(index)
}

/// Expands child names, which may be type names or group names, into ids.
struct ChildResolver<'a> {
	names: &'a FxHashMap<Arc<str>, u16>,
	groups: FxHashMap<Arc<str>, Vec<BlockTypeId>>,
}

impl<'a> ChildResolver<'a> {
	fn new(specs: &[BlockSpec], names: &'a FxHashMap<Arc<str>, u16>) -> Self {
		let mut groups: FxHashMap<Arc<str>, Vec<BlockTypeId>> = FxHashMap::default();

		for (index, spec) in specs.iter().enumerate() {
			for group in &spec.groups {
				groups
					.entry(group.clone())
					.or_default()
					.push(BlockTypeId(index as u16));
			}
		}

		Self { names, groups }
	}

	fn resolve(
		&self,
		owner: &Arc<str>,
		children: &[Arc<str>],
	) -> Result<Vec<BlockTypeId>, SchemaError> {
		let mut ids = Vec::new();

		for child in children {
			ids.extend(self.lookup(owner, child)?);
		}

		ids.sort_unstable();
		ids.dedup();
		Ok(ids)
	}

	fn lookup(&self, owner: &Arc<str>, child: &Arc<str>) -> Result<Vec<BlockTypeId>, SchemaError> {
		if let Some(&id) = self.names.get(child) {
			return Ok(vec![BlockTypeId(id)]);
		}

		self.groups.get(child).cloned().ok_or_else(|| {
			SchemaError::Unknown {
				owner: owner.clone(),
				kind: "block",
				name: child.clone(),
			}
		})
	}
}

fn resolve_block(
	index: usize,
	spec: &BlockSpec,
	resolver: &ChildResolver<'_>,
) -> Result<BlockType, SchemaError> {
	let content = match &spec.content {
		ContentSpec::Inline => ContentKind::Inline,
		ContentSpec::Leaf => ContentKind::Leaf,
		ContentSpec::Blocks(children) => {
			ContentKind::Blocks(resolver.resolve(&spec.name, children)?)
		}
	};

	Ok(BlockType {
		id: BlockTypeId(index as u16),
		name: spec.name.clone(),
		content,
		attrs: spec.attrs.clone(),
	})
}

fn resolve_mark(
	index: usize,
	spec: &MarkSpec,
	names: &FxHashMap<Arc<str>, u16>,
) -> Result<MarkType, SchemaError> {
	let id = MarkTypeId(index as u16);
	let mut excludes = vec![id];

	for name in &spec.excludes {
		let other = names.get(name).ok_or_else(|| {
			SchemaError::Unknown {
				owner: spec.name.clone(),
				kind: "mark",
				name: name.clone(),
			}
		})?;
		excludes.push(MarkTypeId(*other));
	}

	excludes.sort_unstable();
	excludes.dedup();

	Ok(MarkType {
		id,
		name: spec.name.clone(),
		inclusive: spec.inclusive,
		excludes,
		attrs: spec.attrs.clone(),
	})
}

fn resolve_embed(index: usize, spec: EmbedSpec) -> EmbedType {
	EmbedType {
		id: EmbedTypeId(index as u16),
		name: spec.name,
		attrs: spec.attrs,
	}
}

/// A resolved, immutable schema shared by every document that uses it.
#[derive(Clone, Debug)]
pub struct Schema {
	blocks: Vec<BlockType>,
	marks: Vec<MarkType>,
	embeds: Vec<EmbedType>,
	root: Vec<BlockTypeId>,
	default_text_block: BlockTypeId,
	block_names: FxHashMap<Arc<str>, u16>,
	mark_names: FxHashMap<Arc<str>, u16>,
	embed_names: FxHashMap<Arc<str>, u16>,
}

impl Schema {
	/// Start building a schema.
	#[must_use]
	pub fn builder() -> SchemaBuilder {
		SchemaBuilder::new()
	}

	/// The resolved block type for `id`.
	///
	/// # Panics
	///
	/// Panics if `id` came from a different schema with fewer block types.
	#[must_use]
	pub fn block(&self, id: BlockTypeId) -> &BlockType {
		&self.blocks[id.index()]
	}

	/// The resolved mark type for `id`.
	///
	/// # Panics
	///
	/// Panics if `id` came from a different schema with fewer mark types.
	#[must_use]
	pub fn mark(&self, id: MarkTypeId) -> &MarkType {
		&self.marks[id.index()]
	}

	/// The resolved embed type for `id`.
	///
	/// # Panics
	///
	/// Panics if `id` came from a different schema with fewer embed types.
	#[must_use]
	pub fn embed(&self, id: EmbedTypeId) -> &EmbedType {
		&self.embeds[id.index()]
	}

	/// Look up a block type by name.
	#[must_use]
	pub fn block_type(&self, name: &str) -> Option<BlockTypeId> {
		self.block_names.get(name).copied().map(BlockTypeId)
	}

	/// Look up a mark type by name.
	#[must_use]
	pub fn mark_type(&self, name: &str) -> Option<MarkTypeId> {
		self.mark_names.get(name).copied().map(MarkTypeId)
	}

	/// Look up an embed type by name.
	#[must_use]
	pub fn embed_type(&self, name: &str) -> Option<EmbedTypeId> {
		self.embed_names.get(name).copied().map(EmbedTypeId)
	}

	/// All block types, indexed by [`BlockTypeId::index`].
	#[must_use]
	pub fn blocks(&self) -> &[BlockType] {
		&self.blocks
	}

	/// All mark types, indexed by [`MarkTypeId::index`].
	#[must_use]
	pub fn marks(&self) -> &[MarkType] {
		&self.marks
	}

	/// Block types allowed at the document root.
	#[must_use]
	pub fn root_children(&self) -> &[BlockTypeId] {
		&self.root
	}

	/// The text block created for new lines and empty documents: the first
	/// root-allowed block with inline content.
	#[must_use]
	pub fn default_text_block(&self) -> BlockTypeId {
		self.default_text_block
	}

	/// Whether `child` may appear directly inside `parent` (`None` is the
	/// document root).
	#[must_use]
	pub fn allows_child(&self, parent: Option<BlockTypeId>, child: BlockTypeId) -> bool {
		let allowed = match parent {
			None => &self.root,
			Some(parent) => {
				match &self.block(parent).content {
					ContentKind::Blocks(allowed) => allowed,
					ContentKind::Inline | ContentKind::Leaf => return false,
				}
			}
		};

		allowed.binary_search(&child).is_ok()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample() -> Schema {
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.block(BlockSpec::text("heading").attrs(Attrs::new().with("level", 1)))
			.block(BlockSpec::container("bullet_list", &["list_item"]))
			.block(BlockSpec::container("list_item", &["paragraph", "bullet_list"]).groups(&[]))
			.block(BlockSpec::leaf("divider"))
			.mark(MarkSpec::new("bold"))
			.mark(MarkSpec::new("code").excludes(&["bold"]))
			.mark(MarkSpec::new("link").inclusive(false))
			.embed(EmbedSpec::new("mention"))
			.build()
			.expect("sample schema is valid")
	}

	#[test]
	fn resolves_groups_and_names() {
		let schema = sample();
		let list = schema.block_type("bullet_list").expect("list exists");
		let item = schema.block_type("list_item").expect("item exists");
		let paragraph = schema.block_type("paragraph").expect("paragraph exists");

		assert!(schema.allows_child(None, list));
		assert!(
			!schema.allows_child(None, item),
			"list_item is not in the block group"
		);
		assert!(schema.allows_child(Some(list), item));
		assert!(schema.allows_child(Some(item), paragraph));
		assert!(!schema.allows_child(Some(paragraph), paragraph));
		assert_eq!(schema.default_text_block(), paragraph);
	}

	#[test]
	fn marks_always_exclude_themselves() {
		let schema = sample();
		let bold = schema.mark_type("bold").expect("bold exists");
		let code = schema.mark_type("code").expect("code exists");

		assert_eq!(schema.mark(bold).excludes, [bold]);
		assert_eq!(schema.mark(code).excludes, [bold, code]);
		assert!(
			!schema
				.mark(schema.mark_type("link").expect("link exists"))
				.inclusive
		);
	}

	#[test]
	fn rejects_duplicates_and_unknown_references() {
		let duplicate = Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.block(BlockSpec::text("paragraph"))
			.build();
		let unknown = Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.block(BlockSpec::container("list", &["missing"]))
			.build();

		assert!(matches!(
			duplicate,
			Err(SchemaError::Duplicate { kind: "block", .. })
		));
		assert!(matches!(unknown, Err(SchemaError::Unknown { .. })));
	}

	#[test]
	fn requires_a_root_text_block() {
		let schema = Schema::builder().block(BlockSpec::leaf("divider")).build();

		assert_eq!(schema.err(), Some(SchemaError::NoTextBlock));
	}
}
