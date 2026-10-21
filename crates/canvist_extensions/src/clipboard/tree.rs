//! Building schema-valid slices from parsed content.

use canvist_editor::Slice;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::ContentKind;
use canvist_model::Embed;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::MarkTypeId;
use canvist_model::Schema;
use canvist_model::TreeContent;

/// The name of the embed that breaks a line.
pub(super) const HARD_BREAK: &str = "hardBreak";

/// Text with hard breaks as line breaks and other embeds dropped.
pub(super) fn plain_text(schema: &Schema, text: &InlineText) -> String {
	text.runs()
		.map(|run| {
			match run.embed {
				Some(embed) if schema.embed(embed.kind).name.as_ref() == HARD_BREAK => "\n",
				Some(_) => "",
				None => run.text,
			}
		})
		.collect()
}

/// Creates blocks for a parser, resolving TipTap names against the schema
/// and handing out placeholder ids that are unique within the slice.
pub(super) struct Builder<'s> {
	pub(super) schema: &'s Schema,
	next: u32,
}

impl<'s> Builder<'s> {
	pub(super) fn new(schema: &'s Schema) -> Self {
		Self { schema, next: 0 }
	}

	fn id(&mut self) -> BlockId {
		self.next += 1;
		BlockId::new(0, self.next)
	}

	/// A text block of type `name`, or a default text block when the schema
	/// has no such type.
	pub(super) fn text(&mut self, name: &str, attrs: Attrs, inline: InlineText) -> BlockTree {
		let (kind, attrs) = match self.schema.block_type(name) {
			Some(kind) if self.schema.block(kind).content == ContentKind::Inline => (kind, attrs),
			_ => (self.schema.default_text_block(), Attrs::new()),
		};

		let attrs = with_defaults(&self.schema.block(kind).attrs, &attrs);

		BlockTree::text(self.id(), kind, inline).with_attrs(attrs)
	}

	/// A container of type `name`, or its children alone when the schema has
	/// no such type.
	pub(super) fn container(
		&mut self,
		name: &str,
		attrs: &Attrs,
		children: Vec<BlockTree>,
	) -> Vec<BlockTree> {
		let kind = self
			.schema
			.block_type(name)
			.filter(|&kind| matches!(self.schema.block(kind).content, ContentKind::Blocks(_)));
		let Some(kind) = kind else {
			return children;
		};
		let attrs = with_defaults(&self.schema.block(kind).attrs, attrs);

		vec![BlockTree::container(self.id(), kind, children).with_attrs(attrs)]
	}

	/// An atomic block of type `name`, if the schema has one.
	pub(super) fn leaf(&mut self, name: &str) -> Option<BlockTree> {
		let kind = self.schema.block_type(name)?;
		let leaf = self.schema.block(kind).content == ContentKind::Leaf;
		leaf.then(|| BlockTree::leaf(self.id(), kind))
	}

	/// A mark of type `name`, if the schema has one, keeping only the
	/// attributes the type declares. Unset attributes stay absent, as they
	/// are on marks made by commands and typing, so equal marks compare
	/// equal.
	pub(super) fn mark(&self, name: &str, attrs: &Attrs) -> Option<Mark> {
		let kind = self.schema.mark_type(name)?;
		Some(Mark::with_attrs(kind, self.declared(kind, attrs)))
	}

	/// A mark that only means its attributes, such as `textStyle`: `None`
	/// unless at least one attribute is one the type declares, so pasting a
	/// styled `<span>` into an editor without colours adds nothing.
	pub(super) fn style_mark(&self, name: &str, attrs: &Attrs) -> Option<Mark> {
		let kind = self.schema.mark_type(name)?;
		let kept = self.declared(kind, attrs);
		(!kept.is_empty()).then(|| Mark::with_attrs(kind, kept))
	}

	/// The attributes in `attrs` that `kind` declares.
	fn declared(&self, kind: MarkTypeId, attrs: &Attrs) -> Attrs {
		let declared = &self.schema.mark(kind).attrs;
		attrs
			.iter()
			.filter(|(key, _)| declared.get(key).is_some())
			.map(|(key, value)| (key, value.clone()))
			.collect()
	}

	/// A hard break carrying `marks`, if the schema has one.
	pub(super) fn hard_break(&self, marks: MarkSet) -> Option<InlineText> {
		let kind = self.schema.embed_type(HARD_BREAK)?;
		let embed = Embed {
			kind,
			attrs: Attrs::new(),
		};
		Some(InlineText::embed(embed, marks))
	}

	/// Fit `trees` to the schema and wrap them in a slice.
	pub(super) fn finish(mut self, trees: Vec<BlockTree>) -> Slice {
		let fitted = self.fit(None, trees);
		Slice::new(fitted)
	}

	/// Make `trees` valid children of `parent`: keep what is allowed, wrap
	/// what an allowed container accepts (a bare paragraph in a list gets a
	/// list item), retype stray text blocks, flatten other containers, and
	/// drop containers left empty.
	fn fit(&mut self, parent: Option<BlockTypeId>, trees: Vec<BlockTree>) -> Vec<BlockTree> {
		let mut fitted = Vec::new();

		for tree in trees {
			fitted.extend(self.fit_one(parent, tree));
		}

		fitted
	}

	fn fit_one(&mut self, parent: Option<BlockTypeId>, tree: BlockTree) -> Vec<BlockTree> {
		if self.schema.allows_child(parent, tree.kind) {
			return self.fit_inside(tree).into_iter().collect();
		}

		let Some(kind) = self.wrapper(parent, tree.kind) else {
			return self.reshape(parent, tree);
		};
		let attrs = self.schema.block(kind).attrs.clone();
		let wrapped = BlockTree::container(self.id(), kind, vec![tree]).with_attrs(attrs);

		self.fit_inside(wrapped).into_iter().collect()
	}

	fn fit_inside(&mut self, mut tree: BlockTree) -> Option<BlockTree> {
		let TreeContent::Blocks(children) = tree.content else {
			return Some(tree);
		};
		let children = self.fit(Some(tree.kind), children);

		if children.is_empty() {
			return None;
		}

		tree.content = TreeContent::Blocks(children);
		Some(tree)
	}

	/// A container type allowed in `parent` that accepts `child`.
	fn wrapper(&self, parent: Option<BlockTypeId>, child: BlockTypeId) -> Option<BlockTypeId> {
		self.allowed(parent)?
			.iter()
			.copied()
			.find(|&wrapper| self.schema.allows_child(Some(wrapper), child))
	}

	/// The child types `parent` accepts, or `None` if it holds no blocks.
	fn allowed(&self, parent: Option<BlockTypeId>) -> Option<&'s [BlockTypeId]> {
		let Some(parent) = parent else {
			return Some(self.schema.root_children());
		};

		match &self.schema.block(parent).content {
			ContentKind::Blocks(allowed) => Some(allowed),
			ContentKind::Inline | ContentKind::Leaf => None,
		}
	}

	/// Turn a block `parent` rejects into something it accepts.
	fn reshape(&mut self, parent: Option<BlockTypeId>, tree: BlockTree) -> Vec<BlockTree> {
		match tree.content {
			TreeContent::Inline(text) => {
				let Some(kind) = self.text_type(parent) else {
					return Vec::new();
				};
				let attrs = self.schema.block(kind).attrs.clone();
				vec![BlockTree::text(tree.id, kind, text).with_attrs(attrs)]
			}
			TreeContent::Blocks(children) => self.fit(parent, children),
			TreeContent::Leaf => Vec::new(),
		}
	}

	fn text_type(&self, parent: Option<BlockTypeId>) -> Option<BlockTypeId> {
		let Some(parent) = parent else {
			return Some(self.schema.default_text_block());
		};
		let ContentKind::Blocks(allowed) = &self.schema.block(parent).content else {
			return None;
		};

		allowed
			.iter()
			.copied()
			.find(|&kind| self.schema.block(kind).content == ContentKind::Inline)
	}
}

/// `attrs` on top of a type's default attributes.
fn with_defaults(defaults: &Attrs, attrs: &Attrs) -> Attrs {
	let mut merged = defaults.clone();

	for (key, value) in attrs.iter() {
		merged.set(key, value.clone());
	}

	merged
}
