//! Headings, levels 1 to 6.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::InputRule;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Platform;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::RuleMatch;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::resolve;

/// A heading level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HeadingLevel {
	/// `<h1>`.
	H1 = 1,
	/// `<h2>`.
	H2,
	/// `<h3>`.
	H3,
	/// `<h4>`.
	H4,
	/// `<h5>`.
	H5,
	/// `<h6>`.
	H6,
}

impl HeadingLevel {
	/// Every level, from 1 to 6.
	pub const ALL: [Self; 6] = [Self::H1, Self::H2, Self::H3, Self::H4, Self::H5, Self::H6];

	/// The level as a number from 1 to 6.
	#[must_use]
	pub const fn number(self) -> i64 {
		self as i64
	}

	/// The level for a number from 1 to 6.
	#[must_use]
	pub fn from_number(number: i64) -> Option<Self> {
		Self::ALL.into_iter().find(|level| level.number() == number)
	}

	fn attrs(self) -> Attrs {
		Attrs::new().with("level", self.number())
	}
}

/// A `heading` text block with a `level` attribute. `#` to `######`
/// followed by a space turn a paragraph into a heading, and `Mod-Alt-1`
/// to `Mod-Alt-6` toggle one.
#[derive(Clone, Copy, Debug)]
pub struct Heading {
	/// The deepest level offered.
	pub max_level: HeadingLevel,
}

impl Default for Heading {
	fn default() -> Self {
		Self {
			max_level: HeadingLevel::H6,
		}
	}
}

/// Typed access to [`Heading`].
#[derive(Clone, Copy, Debug)]
pub struct HeadingHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HeadingHandle {
	/// Make the selected text blocks headings of `level`, or paragraphs if
	/// they already are.
	#[must_use]
	pub fn toggle(self, level: HeadingLevel) -> Command {
		toggle(level)
	}

	/// Make the selected text blocks headings of `level`.
	#[must_use]
	pub fn set(self, level: HeadingLevel) -> Command {
		set(level)
	}

	/// The level every selected text block shares, if they are all
	/// headings of one level.
	#[must_use]
	pub fn level(self, state: &EditorState) -> Option<HeadingLevel> {
		level(state)
	}
}

fn toggle(level: HeadingLevel) -> Command {
	command(move |state| {
		let heading = resolve::block(state, "heading", "heading")?;
		commands::toggle_block_type(state, heading, &level.attrs())
	})
}

fn set(level: HeadingLevel) -> Command {
	command(move |state| {
		let heading = resolve::block(state, "heading", "heading")?;
		commands::set_block_type(state, heading, &level.attrs())
	})
}

fn level(state: &EditorState) -> Option<HeadingLevel> {
	let heading = state.schema().block_type("heading")?;
	HeadingLevel::ALL
		.into_iter()
		.find(|level| commands::is_block_active(state, heading, &level.attrs()))
}

impl Heading {
	fn levels(self) -> impl Iterator<Item = HeadingLevel> {
		HeadingLevel::ALL
			.into_iter()
			.filter(move |&level| level <= self.max_level)
	}

	fn level_type(self) -> ValueType {
		ValueType::IntRange {
			min: 1,
			max: self.max_level.number(),
		}
	}

	/// Read a checked level argument.
	fn level_arg(args: &canvist_editor::Args<'_>) -> Result<HeadingLevel, EditorError> {
		let number = args.int("level")?;
		HeadingLevel::from_number(number).ok_or_else(|| {
			EditorError::InvalidArgs {
				name: "heading".to_owned(),
				reason: format!("no heading level {number}"),
			}
		})
	}
}

const KEYS: [&str; 6] = [
	"Mod-Alt-1",
	"Mod-Alt-2",
	"Mod-Alt-3",
	"Mod-Alt-4",
	"Mod-Alt-5",
	"Mod-Alt-6",
];

impl Extension for Heading {
	fn name(&self) -> &'static str {
		"heading"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::text("heading").attrs(Attrs::new().with("level", 1)))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let level = Field::optional("level", self.level_type(), 1, "The heading level.");
		vec![NodeMeta::new(NodeKind::Block, "heading").attr(level)]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let level = || Field::required("level", self.level_type(), "The heading level.");
		let toggle_spec = CommandSpec::new(
			"toggleHeading",
			"Make the selected text blocks headings of a level, or paragraphs if they already are.",
			|args| Ok(toggle(Self::level_arg(args)?)),
		);
		let set_spec = CommandSpec::new(
			"setHeading",
			"Make the selected text blocks headings of a level.",
			|args| Ok(set(Self::level_arg(args)?)),
		);
		Ok(vec![toggle_spec.arg(level()), set_spec.arg(level())])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let returns = ValueType::nullable(self.level_type());
		let query = QuerySpec::new(
			"headingLevel",
			"The level every selected text block shares, or null if they are not all headings of \
			 one level.",
			returns,
			|state, _| Ok(level(state).map_or(Value::Null, |level| Value::Int(level.number()))),
		);
		Ok(vec![query])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(self
			.levels()
			.map(|level| (KEYS[level as usize - 1], toggle(level)))
			.collect())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = canvist_editor::block_type(schema, self.name(), "heading")?;
		let pattern = format!(r"^(#{{1,{}}})\s$", self.max_level.number());
		let level = |found: &RuleMatch| {
			Attrs::new().with("level", found.group(1).map_or(1, str::len) as i64)
		};
		Ok(vec![InputRule::block_type(&pattern, kind, level)?])
	}
}

impl TypedExtension for Heading {
	type Handle = HeadingHandle;

	fn handle(&self, registration: Registration) -> HeadingHandle {
		HeadingHandle {
			_registration: registration,
		}
	}
}
