//! Text alignment: a `textAlign` attribute on text blocks.

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

/// The attribute every aligned block carries.
const ATTR: &str = "textAlign";

/// The alignments TipTap offers, which are also CSS `text-align` values.
const ALIGNMENTS: [&str; 4] = ["left", "center", "right", "justify"];

/// The keys TipTap binds, one per alignment.
const KEYS: [(&str, &str); 4] = [
	("Mod-Shift-l", "left"),
	("Mod-Shift-e", "center"),
	("Mod-Shift-r", "right"),
	("Mod-Shift-j", "justify"),
];

/// A `textAlign` attribute on the text blocks named in `types`: left,
/// centre, right, or justified. Layout aligns the lines and server-rendered
/// HTML writes `text-align`. `Mod-Shift-l`, `e`, `r`, and `j` set it.
///
/// An unset alignment follows the text: left, or right for right-to-left
/// paragraphs.
#[derive(Clone, Debug)]
pub struct TextAlign {
	/// The block types that can be aligned. Types the schema lacks are
	/// skipped.
	pub types: Vec<&'static str>,
}

impl Default for TextAlign {
	fn default() -> Self {
		Self {
			types: vec!["heading", "paragraph"],
		}
	}
}

/// Typed access to [`TextAlign`].
#[derive(Clone, Copy, Debug)]
pub struct TextAlignHandle {
	_registration: Registration,
}

/// An alignment for [`TextAlignHandle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Alignment {
	/// The left edge.
	Left,
	/// Centred.
	Center,
	/// The right edge.
	Right,
	/// Both edges.
	Justify,
}

impl Alignment {
	/// The attribute value, such as `center`.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Left => "left",
			Self::Center => "center",
			Self::Right => "right",
			Self::Justify => "justify",
		}
	}

	fn from_name(name: &str) -> Option<Self> {
		[Self::Left, Self::Center, Self::Right, Self::Justify]
			.into_iter()
			.find(|alignment| alignment.name() == name)
	}
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl TextAlignHandle {
	/// Align the selected text blocks.
	#[must_use]
	pub fn set(self, alignment: Alignment) -> Command {
		set(Value::from(alignment.name()))
	}

	/// Clear the alignment of the selected text blocks.
	#[must_use]
	pub fn unset(self) -> Command {
		set(Value::Null)
	}

	/// Clear the alignment if every selected text block has it, otherwise
	/// set it.
	#[must_use]
	pub fn toggle(self, alignment: Alignment) -> Command {
		toggle(alignment.name())
	}

	/// The alignment every selected text block shares, if they share one.
	#[must_use]
	pub fn alignment(self, state: &EditorState) -> Option<Alignment> {
		shared(state).as_deref().and_then(Alignment::from_name)
	}
}

/// The selected text blocks that have the `textAlign` attribute.
fn alignable(state: &EditorState) -> Vec<BlockId> {
	let doc = state.doc();
	let schema = state.schema();

	commands::selected_text_blocks(state)
		.into_iter()
		.filter(|&id| {
			doc.block(id)
				.is_some_and(|block| schema.block(block.kind()).attrs.get(ATTR).is_some())
		})
		.collect()
}

/// The alignment of `block`, or `None` when it has none.
fn alignment_of(state: &EditorState, block: BlockId) -> Option<String> {
	let value = state.doc().block(block)?.attrs().get(ATTR)?;
	value.as_str().map(str::to_owned)
}

/// The alignment every alignable selected block shares.
fn shared(state: &EditorState) -> Option<String> {
	let blocks = alignable(state);
	let first = alignment_of(state, *blocks.first()?)?;

	blocks
		.iter()
		.all(|&block| alignment_of(state, block).as_deref() == Some(&first))
		.then_some(first)
}

/// Set every alignable selected block's alignment to `value`, null to
/// clear it.
fn set_alignment(state: &mut EditorState, value: &Value) -> canvist_core::CommandResult {
	let blocks: Vec<(BlockId, Attrs)> = alignable(state)
		.into_iter()
		.filter_map(|id| {
			let attrs = state.doc().block(id)?.attrs();
			(attrs.get(ATTR).unwrap_or(&Value::Null) != value).then(|| (id, aligned(attrs, value)))
		})
		.collect();

	if blocks.is_empty() {
		return Ok(None);
	}

	state.change(EditKind::Structure, |tr| {
		for (id, attrs) in blocks {
			let kind = tr.doc().block(id).map(canvist_model::Block::kind);

			if let Some(kind) = kind {
				tr.set_block(id, kind, attrs)?;
			}
		}

		Ok(true)
	})
}

/// `attrs` with the alignment `value`. Clearing removes the attribute, as a
/// block that was never aligned does not have it.
fn aligned(attrs: &Attrs, value: &Value) -> Attrs {
	let mut attrs = attrs.clone();

	if *value == Value::Null {
		attrs.remove(ATTR);
	} else {
		attrs.set(ATTR, value.clone());
	}

	attrs
}

fn set(value: Value) -> Command {
	command(move |state| set_alignment(state, &value))
}

fn toggle(alignment: &'static str) -> Command {
	command(move |state| {
		let value = if shared(state).as_deref() == Some(alignment) {
			Value::Null
		} else {
			Value::from(alignment)
		};

		set_alignment(state, &value)
	})
}

fn alignment_field() -> Field {
	Field::required(
		"alignment",
		ValueType::OneOf(ALIGNMENTS.to_vec()),
		"The alignment: left, center, right, or justify.",
	)
}

fn alignment_arg(args: &canvist_core::Args<'_>) -> Result<&'static str, EditorError> {
	let name = args.string("alignment")?;
	ALIGNMENTS
		.into_iter()
		.find(|known| *known == name)
		.ok_or_else(|| {
			EditorError::InvalidArgs {
				name: "textAlign".to_owned(),
				reason: format!("no alignment `{name}`"),
			}
		})
}

impl Extension for TextAlign {
	fn name(&self) -> &'static str {
		ATTR
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block_attrs(&self.types, Attrs::new().with(ATTR, Value::Null))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let field = Field::optional(
			ATTR,
			ValueType::nullable(ValueType::OneOf(ALIGNMENTS.to_vec())),
			Value::Null,
			"How the block's lines align, or null to follow the text direction.",
		);

		self.types
			.iter()
			.map(|&name| NodeMeta::new(NodeKind::Block, name).attr(field.clone()))
			.collect()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let set_spec =
			CommandSpec::new("setTextAlign", "Align the selected text blocks.", |args| {
				Ok(set(Value::from(alignment_arg(args)?)))
			});
		let toggle_spec = CommandSpec::new(
			"toggleTextAlign",
			"Clear the alignment if every selected text block has it, otherwise set it.",
			|args| Ok(toggle(alignment_arg(args)?)),
		);

		Ok(vec![
			set_spec.arg(alignment_field()),
			CommandSpec::simple(
				"unsetTextAlign",
				"Clear the alignment of the selected text blocks.",
				set(Value::Null),
			),
			toggle_spec.arg(alignment_field()),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			ATTR,
			"The alignment every selected text block shares, or null.",
			ValueType::nullable(ValueType::OneOf(ALIGNMENTS.to_vec())),
			|state, _| Ok(shared(state).map_or(Value::Null, Value::from)),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(KEYS
			.into_iter()
			.map(|(key, alignment)| (key, set(Value::from(alignment))))
			.collect())
	}
}

impl TypedExtension for TextAlign {
	type Handle = TextAlignHandle;

	fn handle(&self, registration: Registration) -> TextAlignHandle {
		TextAlignHandle {
			_registration: registration,
		}
	}
}
