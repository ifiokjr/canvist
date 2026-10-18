//! Formatting marks: bold, italic, underline, strike, and inline code.
//!
//! Each mark is described by a [`MarkDef`]: its name, its toggle command
//! and keys, and its Markdown-style input and paste rules. The extensions
//! differ only in that data, so one implementation serves all of them.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::PasteRule;
use canvist_editor::Platform;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::mark_type;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::MarkTypeId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

/// Everything that distinguishes one simple mark from another.
struct MarkDef {
	name: &'static str,
	command: &'static str,
	keys: &'static [&'static str],
	/// Input rule patterns; group 1 is the text to format.
	input: &'static [&'static str],
	/// Paste rule patterns; group 1 is the text to format.
	paste: &'static [&'static str],
}

fn toggle(kind: MarkTypeId) -> Command {
	command(move |state| commands::toggle_mark(state, &Mark::new(kind)))
}

impl MarkDef {
	fn kind(&self, schema: &Schema) -> Result<MarkTypeId, EditorError> {
		mark_type(schema, self.name, self.name)
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		Ok(vec![(self.command, toggle(self.kind(schema)?))])
	}

	fn keymap(&self, schema: &Schema) -> Result<Vec<Binding>, EditorError> {
		let kind = self.kind(schema)?;
		Ok(self.keys.iter().map(|&key| (key, toggle(kind))).collect())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = self.kind(schema)?;
		self.input
			.iter()
			.map(|pattern| InputRule::mark(pattern, kind))
			.collect()
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		let kind = self.kind(schema)?;
		self.paste
			.iter()
			.map(|pattern| PasteRule::mark(pattern, kind))
			.collect()
	}
}

#[cfg(any(test, feature = "bold"))]
const BOLD: MarkDef = MarkDef {
	name: "bold",
	command: "toggleBold",
	keys: &["Mod-b"],
	input: &[
		r"(?:^|\s)\*\*([^*\s](?:[^*]*[^*\s])?)\*\*$",
		r"(?:^|\s)__([^_\s](?:[^_]*[^_\s])?)__$",
	],
	paste: &[
		r"(?:^|\s)\*\*([^*\s](?:[^*]*[^*\s])?)\*\*",
		r"(?:^|\s)__([^_\s](?:[^_]*[^_\s])?)__",
	],
};

#[cfg(any(test, feature = "italic"))]
const ITALIC: MarkDef = MarkDef {
	name: "italic",
	command: "toggleItalic",
	keys: &["Mod-i"],
	input: &[
		r"(?:^|\s)\*([^*\s](?:[^*]*[^*\s])?)\*$",
		r"(?:^|\s)_([^_\s](?:[^_]*[^_\s])?)_$",
	],
	paste: &[
		r"(?:^|\s)\*([^*\s](?:[^*]*[^*\s])?)\*",
		r"(?:^|\s)_([^_\s](?:[^_]*[^_\s])?)_",
	],
};

#[cfg(any(test, feature = "underline"))]
const UNDERLINE: MarkDef = MarkDef {
	name: "underline",
	command: "toggleUnderline",
	keys: &["Mod-u"],
	input: &[],
	paste: &[],
};

#[cfg(any(test, feature = "strike"))]
const STRIKE: MarkDef = MarkDef {
	name: "strike",
	command: "toggleStrike",
	keys: &["Mod-Shift-s"],
	input: &[r"(?:^|\s)~~([^~\s](?:[^~]*[^~\s])?)~~$"],
	paste: &[r"(?:^|\s)~~([^~\s](?:[^~]*[^~\s])?)~~"],
};

#[cfg(any(test, feature = "code"))]
const CODE: MarkDef = MarkDef {
	name: "code",
	command: "toggleCode",
	keys: &["Mod-e"],
	input: &[r"(?:^|\s)`([^`\s](?:[^`]*[^`\s])?)`$"],
	paste: &[r"(?:^|\s)`([^`\s](?:[^`]*[^`\s])?)`"],
};

/// Implements [`Extension`] for a mark type by delegating to its
/// [`MarkDef`]; only the schema spec differs per type.
macro_rules! delegate_to_def {
	($def:expr) => {
		fn name(&self) -> &'static str {
			$def.name
		}

		fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
			$def.commands(schema)
		}

		fn keymap(
			&self,
			schema: &Schema,
			_platform: Platform,
		) -> Result<Vec<Binding>, EditorError> {
			$def.keymap(schema)
		}

		fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
			$def.input_rules(schema)
		}

		fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
			$def.paste_rules(schema)
		}
	};
}

/// Bold text: `Mod-b`, `**text**`, or `__text__`.
#[cfg(feature = "bold")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Bold;

#[cfg(feature = "bold")]
impl Extension for Bold {
	delegate_to_def!(BOLD);

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("bold"))
	}
}

/// Italic text: `Mod-i`, `*text*`, or `_text_`.
#[cfg(feature = "italic")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Italic;

#[cfg(feature = "italic")]
impl Extension for Italic {
	delegate_to_def!(ITALIC);

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("italic"))
	}
}

/// Underlined text: `Mod-u`.
#[cfg(feature = "underline")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Underline;

#[cfg(feature = "underline")]
impl Extension for Underline {
	delegate_to_def!(UNDERLINE);

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("underline"))
	}
}

/// Struck-through text: `Mod-Shift-s` or `~~text~~`.
#[cfg(feature = "strike")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Strike;

#[cfg(feature = "strike")]
impl Extension for Strike {
	delegate_to_def!(STRIKE);

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("strike"))
	}
}

/// Inline code: `Mod-e` or `` `text` ``.
///
/// Code usually excludes other formatting. The schema can only exclude
/// marks it knows, so list them in `excludes`; the StarterKit fills it with
/// the marks it registers.
#[cfg(feature = "code")]
#[derive(Clone, Debug, Default)]
pub struct Code {
	/// Marks that cannot coexist with code.
	pub excludes: Vec<&'static str>,
}

#[cfg(feature = "code")]
impl Extension for Code {
	delegate_to_def!(CODE);

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("code").excludes(&self.excludes))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_pattern_compiles() {
		let schema = Schema::builder()
			.block(canvist_model::BlockSpec::text("paragraph"))
			.mark(MarkSpec::new("bold"))
			.mark(MarkSpec::new("italic"))
			.mark(MarkSpec::new("underline"))
			.mark(MarkSpec::new("strike"))
			.mark(MarkSpec::new("code"))
			.build()
			.expect("valid schema");

		for def in [BOLD, ITALIC, UNDERLINE, STRIKE, CODE] {
			assert!(def.input_rules(&schema).is_ok(), "{}", def.name);
			assert!(def.paste_rules(&schema).is_ok(), "{}", def.name);
		}
	}
}
