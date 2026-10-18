//! Headings, levels 1 to 6.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::Platform;
use canvist_editor::RuleMatch;
use canvist_editor::block_type;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::BlockTypeId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

const NAMES: [&str; 6] = [
	"toggleHeading1",
	"toggleHeading2",
	"toggleHeading3",
	"toggleHeading4",
	"toggleHeading5",
	"toggleHeading6",
];
const KEYS: [&str; 6] = [
	"Mod-Alt-1",
	"Mod-Alt-2",
	"Mod-Alt-3",
	"Mod-Alt-4",
	"Mod-Alt-5",
	"Mod-Alt-6",
];

/// A `heading` text block with a `level` attribute. `#` to `######`
/// followed by a space turn a paragraph into a heading, and `Mod-Alt-1`
/// to `Mod-Alt-6` toggle one.
#[derive(Clone, Copy, Debug)]
pub struct Heading {
	/// The deepest level offered, from 1 to 6.
	pub max_level: u8,
}

impl Default for Heading {
	fn default() -> Self {
		Self { max_level: 6 }
	}
}

impl Heading {
	fn levels(self) -> std::ops::RangeInclusive<usize> {
		1..=usize::from(self.max_level.clamp(1, 6))
	}

	fn kind(schema: &Schema) -> Result<BlockTypeId, EditorError> {
		block_type(schema, "heading", "heading")
	}

	fn toggle(kind: BlockTypeId, level: usize) -> Command {
		let attrs = Attrs::new().with("level", level as i64);
		command(move |state| commands::toggle_block_type(state, kind, &attrs))
	}
}

impl Extension for Heading {
	fn name(&self) -> &'static str {
		"heading"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::text("heading").attrs(Attrs::new().with("level", 1)))
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		let kind = Self::kind(schema)?;
		Ok(self
			.levels()
			.map(|level| (NAMES[level - 1], Self::toggle(kind, level)))
			.collect())
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		let kind = Self::kind(schema)?;
		Ok(self
			.levels()
			.map(|level| (KEYS[level - 1], Self::toggle(kind, level)))
			.collect())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = Self::kind(schema)?;
		let pattern = format!(r"^(#{{1,{}}})\s$", self.max_level.clamp(1, 6));
		let level = |found: &RuleMatch| {
			Attrs::new().with("level", found.group(1).map_or(1, str::len) as i64)
		};
		Ok(vec![InputRule::block_type(&pattern, kind, level)?])
	}
}
