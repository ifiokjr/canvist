//! Formatting marks: bold, italic, underline, strike, inline code,
//! subscript, and superscript.
//!
//! Each mark is described by a [`MarkDef`]: its name, its toggle command
//! and keys, and its Markdown-style input and paste rules. The extensions
//! differ only in that data, so one implementation serves all of them.

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::PasteRule;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::resolve;

/// Everything that distinguishes one simple mark from another.
#[derive(Debug)]
struct MarkDef {
	name: &'static str,
	/// Command and query names: toggle, set, unset, and the active query.
	names: [&'static str; 4],
	keys: &'static [&'static str],
	/// Input rule patterns; group 1 is the text to format.
	input: &'static [&'static str],
	/// Paste rule patterns; group 1 is the text to format.
	paste: &'static [&'static str],
}

/// Typed access to a mark extension such as [`Bold`].
#[derive(Clone, Copy, Debug)]
pub struct MarkHandle {
	def: &'static MarkDef,
	_registration: Registration,
}

impl MarkHandle {
	/// Add the mark to the selection, or remove it if every selected
	/// character has it. At a caret, it applies to the next typed text.
	#[must_use]
	pub fn toggle(self) -> Command {
		self.def.toggle()
	}

	/// Add the mark to the selection.
	#[must_use]
	pub fn set(self) -> Command {
		self.def.set()
	}

	/// Remove the mark from the selection.
	#[must_use]
	pub fn unset(self) -> Command {
		self.def.unset()
	}

	/// Whether the mark is active at the selection.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		resolve::mark_active(state, self.def.name)
	}
}

impl MarkDef {
	fn toggle(&'static self) -> Command {
		command(move |state| {
			let kind = resolve::mark(state, self.name, self.name)?;
			commands::toggle_mark(state, &Mark::new(kind))
		})
	}

	fn set(&'static self) -> Command {
		command(move |state| {
			let kind = resolve::mark(state, self.name, self.name)?;
			commands::set_mark(state, &Mark::new(kind))
		})
	}

	fn unset(&'static self) -> Command {
		command(move |state| {
			let kind = resolve::mark(state, self.name, self.name)?;
			commands::unset_mark(state, kind)
		})
	}

	fn node(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, self.name)]
	}

	fn commands(&'static self) -> Vec<CommandSpec> {
		let [toggle, set, unset, _] = self.names;
		vec![
			CommandSpec::simple(
				toggle,
				"Add the mark to the selection, or remove it if every selected character has it.",
				self.toggle(),
			),
			CommandSpec::simple(set, "Add the mark to the selection.", self.set()),
			CommandSpec::simple(unset, "Remove the mark from the selection.", self.unset()),
		]
	}

	fn queries(&'static self) -> Vec<QuerySpec> {
		let name = self.name;
		vec![QuerySpec::new(
			self.names[3],
			"Whether the mark is active at the selection.",
			ValueType::Bool,
			move |state, _| Ok(Value::Bool(resolve::mark_active(state, name))),
		)]
	}

	fn keymap(&'static self) -> Vec<Binding> {
		self.keys.iter().map(|&key| (key, self.toggle())).collect()
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = mark_type(schema, self.name, self.name)?;
		self.input
			.iter()
			.map(|pattern| InputRule::mark(pattern, kind))
			.collect()
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		let kind = mark_type(schema, self.name, self.name)?;
		self.paste
			.iter()
			.map(|pattern| PasteRule::mark(pattern, kind))
			.collect()
	}

	fn handle(&'static self, registration: Registration) -> MarkHandle {
		MarkHandle {
			def: self,
			_registration: registration,
		}
	}
}

#[cfg(any(test, feature = "bold"))]
static BOLD: MarkDef = MarkDef {
	name: "bold",
	names: ["toggleBold", "setBold", "unsetBold", "isBold"],
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
static ITALIC: MarkDef = MarkDef {
	name: "italic",
	names: ["toggleItalic", "setItalic", "unsetItalic", "isItalic"],
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
static UNDERLINE: MarkDef = MarkDef {
	name: "underline",
	names: [
		"toggleUnderline",
		"setUnderline",
		"unsetUnderline",
		"isUnderline",
	],
	keys: &["Mod-u"],
	input: &[],
	paste: &[],
};

#[cfg(any(test, feature = "strike"))]
static STRIKE: MarkDef = MarkDef {
	name: "strike",
	names: ["toggleStrike", "setStrike", "unsetStrike", "isStrike"],
	keys: &["Mod-Shift-s"],
	input: &[r"(?:^|\s)~~([^~\s](?:[^~]*[^~\s])?)~~$"],
	paste: &[r"(?:^|\s)~~([^~\s](?:[^~]*[^~\s])?)~~"],
};

#[cfg(any(test, feature = "code"))]
static CODE: MarkDef = MarkDef {
	name: "code",
	names: ["toggleCode", "setCode", "unsetCode", "isCode"],
	keys: &["Mod-e"],
	input: &[r"(?:^|\s)`([^`\s](?:[^`]*[^`\s])?)`$"],
	paste: &[r"(?:^|\s)`([^`\s](?:[^`]*[^`\s])?)`"],
};

#[cfg(any(test, feature = "subscript"))]
static SUBSCRIPT: MarkDef = MarkDef {
	name: "subscript",
	names: [
		"toggleSubscript",
		"setSubscript",
		"unsetSubscript",
		"isSubscript",
	],
	keys: &["Mod-,"],
	input: &[],
	paste: &[],
};

#[cfg(any(test, feature = "superscript"))]
static SUPERSCRIPT: MarkDef = MarkDef {
	name: "superscript",
	names: [
		"toggleSuperscript",
		"setSuperscript",
		"unsetSuperscript",
		"isSuperscript",
	],
	keys: &["Mod-."],
	input: &[],
	paste: &[],
};

/// Implements [`Extension`] and [`TypedExtension`] for a mark type by
/// delegating to its [`MarkDef`]; only the schema spec differs per type.
macro_rules! delegate_to_def {
	($type:ty, $def:expr) => {
		impl TypedExtension for $type {
			type Handle = MarkHandle;

			fn handle(&self, registration: Registration) -> MarkHandle {
				$def.handle(registration)
			}
		}
	};
	($def:expr) => {
		fn name(&self) -> &'static str {
			$def.name
		}

		fn nodes(&self) -> Vec<NodeMeta> {
			$def.node()
		}

		fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
			Ok($def.commands())
		}

		fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
			Ok($def.queries())
		}

		fn keymap(
			&self,
			_schema: &Schema,
			_platform: Platform,
		) -> Result<Vec<Binding>, EditorError> {
			Ok($def.keymap())
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

#[cfg(feature = "bold")]
delegate_to_def!(Bold, BOLD);

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

#[cfg(feature = "italic")]
delegate_to_def!(Italic, ITALIC);

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

#[cfg(feature = "underline")]
delegate_to_def!(Underline, UNDERLINE);

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

#[cfg(feature = "strike")]
delegate_to_def!(Strike, STRIKE);

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

#[cfg(feature = "code")]
delegate_to_def!(Code, CODE);

/// Subscript text: `Mod-,`. Subscript and superscript exclude each other.
#[cfg(feature = "subscript")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Subscript;

#[cfg(feature = "subscript")]
impl Extension for Subscript {
	delegate_to_def!(SUBSCRIPT);

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("subscript").excludes_registered(&["superscript"]))
	}
}

#[cfg(feature = "subscript")]
delegate_to_def!(Subscript, SUBSCRIPT);

/// Superscript text: `Mod-.`. Subscript and superscript exclude each other.
#[cfg(feature = "superscript")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Superscript;

#[cfg(feature = "superscript")]
impl Extension for Superscript {
	delegate_to_def!(SUPERSCRIPT);

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("superscript").excludes_registered(&["subscript"]))
	}
}

#[cfg(feature = "superscript")]
delegate_to_def!(Superscript, SUPERSCRIPT);

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
			.mark(MarkSpec::new("subscript"))
			.mark(MarkSpec::new("superscript"))
			.build()
			.expect("valid schema");

		for def in [
			&BOLD,
			&ITALIC,
			&UNDERLINE,
			&STRIKE,
			&CODE,
			&SUBSCRIPT,
			&SUPERSCRIPT,
		] {
			assert!(def.input_rules(&schema).is_ok(), "{}", def.name);
			assert!(def.paste_rules(&schema).is_ok(), "{}", def.name);
		}
	}
}
