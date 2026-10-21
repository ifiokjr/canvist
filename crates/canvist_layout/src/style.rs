//! Resolving a [`Theme`] against a [`Schema`].
//!
//! Rules name types by string; [`Styles`] compiles them once per (theme,
//! schema) into per-type rule lists, then interns every distinct resolved
//! [`BlockStyle`] so rows and caches compare styles by a small id.

use std::sync::Arc;

use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockTypeId;
use canvist_model::EmbedTypeId;
use canvist_model::MarkSet;
use canvist_model::Schema;
use canvist_model::Value;
use rustc_hash::FxHashMap;

use crate::Color;
use crate::Insets;
use crate::theme::AttrBinding;
use crate::theme::BlockPatch;
use crate::theme::BlockRule;
use crate::theme::Border;
use crate::theme::EmbedStyle;
use crate::theme::LeafStyle;
use crate::theme::MarkRule;
use crate::theme::MarkerKind;
use crate::theme::TableRole;
use crate::theme::TextStyle;
use crate::theme::Theme;

/// The fully resolved style of one block in its context.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockStyle {
	/// The text style of the block's inline content, inherited by children.
	pub text: TextStyle,
	/// Space between the border and the content.
	pub padding: Insets,
	/// The left border, if any.
	pub border_left: Option<Border>,
	/// The background, if any.
	pub background: Option<Color>,
	/// The background's corner radius.
	pub radius: f32,
	/// Space requested above the block.
	pub space_before: f32,
	/// Space requested below the block.
	pub space_after: f32,
	/// The marker drawn beside each child.
	pub marker: Option<MarkerKind>,
	/// How the block draws when it has no content.
	pub leaf: LeafStyle,
	/// Its part in a table grid, if any.
	pub table: Option<TableRole>,
	/// The HTML element for server rendering.
	pub tag: Arc<str>,
}

impl BlockStyle {
	/// Distance from the border box's left edge to the content box.
	#[must_use]
	pub fn inset_left(&self) -> f32 {
		self.border_left.map_or(0.0, |border| border.width) + self.padding.left
	}

	/// Distance from the content box's right edge to the border box.
	#[must_use]
	pub fn inset_right(&self) -> f32 {
		self.padding.right
	}
}

/// An interned [`BlockStyle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct StyleId(u32);

impl StyleId {
	/// The style's position in its [`Styles`].
	pub(crate) const fn index(self) -> u32 {
		self.0
	}
}

/// How many of one block type's rules can match. Matches are recorded as a
/// `u64` bitmask in [`StyleKey`], so rules past this count are ignored.
const MAX_RULES_PER_TYPE: usize = u64::BITS as usize;

/// A block rule bound to schema ids.
#[derive(Clone, Debug)]
struct CompiledRule {
	/// Index into [`Theme::block_rules`].
	rule: usize,
	/// The required parent type, if the rule has one.
	parent: Option<BlockTypeId>,
}

/// What makes two blocks resolve to the same style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct StyleKey {
	kind: BlockTypeId,
	parent: Option<StyleId>,
	/// Which of the type's rules matched, by position.
	matched: u64,
}

/// A theme compiled for one schema.
#[derive(Clone, Debug)]
pub(crate) struct Styles {
	theme: Arc<Theme>,
	schema: Arc<Schema>,
	rules: Vec<Vec<CompiledRule>>,
	marks: Vec<Option<usize>>,
	embeds: FxHashMap<EmbedTypeId, usize>,
	resolved: Vec<BlockStyle>,
	interned: FxHashMap<StyleKey, StyleId>,
	/// Device pixels per CSS pixel that border widths snap to, if any.
	border_scale: Option<f32>,
}

impl Styles {
	/// Snap border widths down to whole multiples of `1 / scale` px (at
	/// least one), as browsers snap borders to device pixels.
	pub(crate) fn with_border_scale(mut self, scale: Option<f32>) -> Self {
		self.border_scale = scale.filter(|scale| scale.is_finite() && *scale > 0.0);
		self
	}

	/// Compile `theme` for `schema`. Rules naming types the schema lacks are
	/// ignored, so one theme serves apps with different extension sets.
	pub(crate) fn new(theme: Arc<Theme>, schema: Arc<Schema>) -> Self {
		let rules = compile_block_rules(&theme, &schema);
		let marks = compile_mark_rules(&theme, &schema);
		let embeds = theme
			.embed_rules()
			.iter()
			.enumerate()
			.filter_map(|(index, rule)| Some((schema.embed_type(&rule.embed)?, index)))
			.collect();

		Self {
			theme,
			schema,
			rules,
			marks,
			embeds,
			border_scale: None,
			resolved: Vec::new(),
			interned: FxHashMap::default(),
		}
	}

	pub(crate) fn theme(&self) -> &Arc<Theme> {
		&self.theme
	}

	pub(crate) fn schema(&self) -> &Arc<Schema> {
		&self.schema
	}

	/// The style with `id`.
	pub(crate) fn get(&self, id: StyleId) -> &BlockStyle {
		&self.resolved[id.0 as usize]
	}

	/// Resolve `block` inside a parent of type `parent_kind` whose style is
	/// `parent`. Both are `None` at the document root.
	pub(crate) fn resolve(
		&mut self,
		block: &Block,
		parent_kind: Option<BlockTypeId>,
		parent: Option<StyleId>,
	) -> StyleId {
		let kind = block.kind();
		let matched = self.matched_rules(block, parent_kind);
		let key = StyleKey {
			kind,
			parent,
			matched,
		};

		if let Some(&id) = self.interned.get(&key) {
			return id;
		}

		let style = self.build(key);
		let index = self
			.resolved
			.iter()
			.position(|existing| *existing == style)
			.unwrap_or_else(|| {
				self.resolved.push(style);
				self.resolved.len() - 1
			});
		let id = StyleId(index as u32);
		self.interned.insert(key, id);
		id
	}

	fn matched_rules(&self, block: &Block, parent_kind: Option<BlockTypeId>) -> u64 {
		let Some(rules) = self.rules.get(block.kind().index()) else {
			return 0;
		};
		let defaults = &self.schema.block(block.kind()).attrs;
		let theme_rules = self.theme.block_rules();

		rules
			.iter()
			.take(MAX_RULES_PER_TYPE)
			.enumerate()
			.filter(|(_, compiled)| {
				let rule = &theme_rules[compiled.rule];
				let parent_ok = compiled.parent.is_none() || compiled.parent == parent_kind;
				let attr_ok = rule
					.when
					.as_ref()
					.is_none_or(|(key, value)| attr(block.attrs(), defaults, key) == Some(value));
				parent_ok && attr_ok
			})
			.fold(0, |mask, (bit, _)| mask | (1 << bit))
	}

	fn build(&self, key: StyleKey) -> BlockStyle {
		let text = match key.parent {
			Some(parent) => self.get(parent).text.clone(),
			None => self.theme.base().clone(),
		};

		let mut style = BlockStyle {
			text,
			padding: Insets::ZERO,
			border_left: None,
			background: None,
			radius: 0.0,
			space_before: 0.0,
			space_after: 0.0,
			marker: None,
			leaf: LeafStyle::Slot { height: 0.0 },
			table: None,
			tag: Arc::from("div"),
		};
		let Some(rules) = self.rules.get(key.kind.index()) else {
			return style;
		};
		let matched = rules
			.iter()
			.take(MAX_RULES_PER_TYPE)
			.enumerate()
			.filter(|(bit, _)| key.matched & (1 << bit) != 0);

		for (_, compiled) in matched {
			apply_block_patch(&mut style, &self.theme.block_rules()[compiled.rule].patch);
		}

		if let (Some(scale), Some(border)) = (self.border_scale, style.border_left.as_mut()) {
			border.width = snap_border(border.width, scale);
		}

		if let (Some(scale), Some(TableRole::Table { rule })) =
			(self.border_scale, style.table.as_mut())
		{
			rule.width = snap_border(rule.width, scale);
		}

		style
	}

	/// The text style of a run with `marks` inside a block styled `base`.
	pub(crate) fn run_style(&self, base: &TextStyle, marks: &MarkSet) -> TextStyle {
		let mut style = base.clone();

		for mark in marks.as_slice() {
			let Some(rule) = self
				.marks
				.get(mark.kind.index())
				.copied()
				.flatten()
				.map(|index| &self.theme.mark_rules()[index])
			else {
				continue;
			};

			rule.patch.apply(&mut style);

			for binding in &rule.bindings {
				apply_binding(&mut style, binding, &mark.attrs);
			}
		}

		style
	}

	/// Every interned style with its id, in interning order.
	pub(crate) fn all(&self) -> impl Iterator<Item = (StyleId, &BlockStyle)> {
		self.resolved
			.iter()
			.enumerate()
			.map(|(index, style)| (StyleId(index as u32), style))
	}

	/// The theme rule for a mark type.
	pub(crate) fn mark_rule(&self, kind: canvist_model::MarkTypeId) -> Option<&MarkRule> {
		let index = self.marks.get(kind.index()).copied().flatten()?;
		Some(&self.theme.mark_rules()[index])
	}

	/// The HTML tag for a mark type, if the theme has a rule for it.
	pub(crate) fn mark_tag(&self, kind: canvist_model::MarkTypeId) -> Option<&Arc<str>> {
		self.mark_rule(kind).map(|rule| &rule.tag)
	}

	/// How embeds of `kind` lay out, if the theme styles them.
	pub(crate) fn embed_style(&self, kind: EmbedTypeId) -> Option<&EmbedStyle> {
		let index = self.embeds.get(&kind)?;
		Some(&self.theme.embed_rules()[*index].style)
	}
}

/// Each block type's rules bound to schema ids, indexed by type.
fn compile_block_rules(theme: &Theme, schema: &Schema) -> Vec<Vec<CompiledRule>> {
	let mut rules = vec![Vec::new(); schema.blocks().len()];
	let compiled = theme
		.block_rules()
		.iter()
		.enumerate()
		.filter_map(|(index, rule)| compile_block_rule(schema, index, rule));

	for (kind, rule) in compiled {
		if let Some(list) = rules.get_mut(kind.index()) {
			list.push(rule);
		}
	}

	rules
}

/// Bind the theme's block rule at `index` to schema ids, or `None` when the
/// schema lacks its block type or its required parent type, since such a rule
/// can never match.
fn compile_block_rule(
	schema: &Schema,
	index: usize,
	rule: &BlockRule,
) -> Option<(BlockTypeId, CompiledRule)> {
	let kind = schema.block_type(&rule.block)?;

	let parent = match &rule.parent {
		Some(name) => Some(schema.block_type(name)?),
		None => None,
	};

	Some((
		kind,
		CompiledRule {
			rule: index,
			parent,
		},
	))
}

/// The index of each mark type's winning theme rule, indexed by type.
fn compile_mark_rules(theme: &Theme, schema: &Schema) -> Vec<Option<usize>> {
	let mut marks = vec![None; schema.marks().len()];

	for (index, rule) in theme.mark_rules().iter().enumerate() {
		if let Some(slot) = schema
			.mark_type(&rule.mark)
			.and_then(|kind| marks.get_mut(kind.index()))
		{
			*slot = Some(index);
		}
	}

	marks
}

/// Look up `key` on a block, falling back to its type's default attributes.
pub(crate) fn attr<'a>(attrs: &'a Attrs, defaults: &'a Attrs, key: &str) -> Option<&'a Value> {
	attrs
		.get(key)
		.filter(|value| **value != Value::Null)
		.or_else(|| defaults.get(key))
}

/// A border width as a browser draws it at `scale` device pixels per CSS
/// pixel: rounded down to whole device pixels, but never below one.
fn snap_border(width: f32, scale: f32) -> f32 {
	if width <= 0.0 {
		return 0.0;
	}

	(width * scale).floor().max(1.0) / scale
}

/// A numeric attribute as pixels.
pub(crate) fn attr_px(attrs: &Attrs, key: &str) -> Option<f32> {
	match attrs.get(key)? {
		Value::Int(value) => Some(*value as f32),
		Value::Float(value) => Some(*value as f32),
		Value::String(value) => value.trim_end_matches("px").parse().ok(),
		_ => None,
	}
	.filter(|px: &f32| px.is_finite() && *px >= 0.0)
}

/// Layer `patch` over `style`: every field the patch sets wins.
fn apply_block_patch(style: &mut BlockStyle, patch: &BlockPatch) {
	patch.text.apply(&mut style.text);

	override_with(&mut style.padding, patch.padding);
	override_with(&mut style.radius, patch.radius);
	override_with(&mut style.space_before, patch.space_before);
	override_with(&mut style.space_after, patch.space_after);
	override_with(&mut style.leaf, patch.leaf);

	style.border_left = patch.border_left.or(style.border_left);
	style.background = patch.background.or(style.background);
	style.marker = patch.marker.or(style.marker);
	style.table = patch.table.or(style.table);

	if let Some(tag) = &patch.tag {
		style.tag = tag.clone();
	}
}

fn override_with<T>(target: &mut T, value: Option<T>) {
	if let Some(value) = value {
		*target = value;
	}
}

/// Apply one of a mark rule's attribute bindings. A missing or malformed
/// attribute leaves the style unchanged, so bad data never breaks rendering.
fn apply_binding(style: &mut TextStyle, binding: &AttrBinding, attrs: &Attrs) {
	match binding {
		AttrBinding::Color(key) => override_with(&mut style.color, hex_attr(attrs, key)),
		AttrBinding::Background(key) => {
			override_with(&mut style.background, hex_attr(attrs, key).map(Some));
		}
		AttrBinding::FontFamily(key) => {
			let family = attrs.get(key).and_then(Value::as_str).map(Arc::from);
			override_with(&mut style.font.family, family);
		}
		AttrBinding::FontSize(key) => {
			let size = attr_px(attrs, key).filter(|size| *size > 0.0);
			override_with(&mut style.font.size, size);
		}
		AttrBinding::LineHeight(key) => {
			override_with(&mut style.line_height, line_height(attrs, key));
		}
	}
}

/// A unitless line height attribute, as a number or a numeric string.
pub(crate) fn line_height(attrs: &Attrs, key: &str) -> Option<f32> {
	match attrs.get(key)? {
		Value::Int(value) => Some(*value as f32),
		Value::Float(value) => Some(*value as f32),
		Value::String(value) => value.trim().parse().ok(),
		_ => None,
	}
	.filter(|height: &f32| height.is_finite() && *height > 0.0)
}

/// A `#rrggbb` attribute as a colour.
fn hex_attr(attrs: &Attrs, key: &str) -> Option<Color> {
	attrs
		.get(key)
		.and_then(Value::as_str)
		.and_then(Color::parse_hex)
}

#[cfg(test)]
#[allow(
	clippy::float_cmp,
	reason = "the snapped widths are exact multiples of a device pixel"
)]
mod tests {
	use super::snap_border;

	#[test]
	fn borders_snap_down_to_device_pixels() {
		assert_eq!(snap_border(3.0, 1.0), 3.0);
		assert_eq!(snap_border(3.0, 1.25), 2.4, "3.75 device pixels draw as 3");
		assert_eq!(snap_border(3.0, 2.0), 3.0);
		assert_eq!(
			snap_border(0.5, 1.0),
			1.0,
			"never thinner than one device pixel"
		);
		assert_eq!(snap_border(0.0, 1.5), 0.0);
	}
}
