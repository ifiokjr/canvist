//! Code blocks.

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::RuleMatch;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::block_type;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::BlockTypeId;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

use crate::resolve;

/// A `codeBlock` text block with a `language` attribute.
///
/// Enter inserts a line break; pressing it on a third empty line leaves the
/// block. ```` ``` ```` or `~~~` followed by an optional language and a
/// space starts one, `Mod-Alt-c` toggles one, and Backspace in an empty one
/// turns it back into a paragraph. Input and paste rules do not run inside.
#[derive(Clone, Copy, Debug, Default)]
pub struct CodeBlock;

impl CodeBlock {
	fn kind(schema: &Schema) -> Result<BlockTypeId, EditorError> {
		block_type(schema, "codeBlock", "codeBlock")
	}
}

/// Typed access to [`CodeBlock`].
#[derive(Clone, Copy, Debug)]
pub struct CodeBlockHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl CodeBlockHandle {
	/// Make the selected text blocks code in `language`, or paragraphs if
	/// they already are code.
	#[must_use]
	pub fn toggle(self, language: Option<&str>) -> Command {
		toggle(language)
	}

	/// Make the selected text blocks code in `language`.
	#[must_use]
	pub fn set(self, language: Option<&str>) -> Command {
		set(language)
	}

	/// Whether the selection is in a code block.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		resolve::around(state, "codeBlock").is_some()
	}

	/// The language of the code block holding the selection.
	#[must_use]
	pub fn language(self, state: &EditorState) -> Option<String> {
		language_at(state)
	}
}

fn language_attrs(language: Option<&str>) -> Attrs {
	Attrs::new().with("language", language.map_or(Value::Null, Value::from))
}

fn toggle(language: Option<&str>) -> Command {
	let attrs = language_attrs(language);
	command(move |state| {
		let code = resolve::block(state, "codeBlock", "codeBlock")?;
		commands::toggle_block_type(state, code, &attrs)
	})
}

fn set(language: Option<&str>) -> Command {
	let attrs = language_attrs(language);
	command(move |state| {
		let code = resolve::block(state, "codeBlock", "codeBlock")?;
		commands::set_block_type(state, code, &attrs)
	})
}

fn language_at(state: &EditorState) -> Option<String> {
	let language = resolve::attr_around(state, "codeBlock", "language")?;
	language.as_str().map(str::to_owned)
}

fn language_field() -> Field {
	let ty = ValueType::nullable(ValueType::String);
	Field::optional(
		"language",
		ty,
		Value::Null,
		"The code's language, such as `rust`.",
	)
}

/// The caret, if it sits in a code block.
fn caret_in(state: &EditorState, kind: BlockTypeId) -> Option<Point> {
	let caret = state.selection().as_caret()?;
	let block = state.doc().block(caret.block)?;
	(block.kind() == kind).then_some(caret)
}

/// Enter inside a code block: a line break, or leave the block when the
/// caret ends two empty lines.
fn enter(state: &mut EditorState, kind: BlockTypeId) -> CommandResult {
	let Some(caret) = caret_in(state, kind) else {
		return Ok(None);
	};
	let text = state
		.doc()
		.inline(caret.block)
		.map_or("", |text| text.as_str());
	let at_end = caret.offset as usize == text.len();

	if !(at_end && text.ends_with("\n\n")) {
		return commands::insert_text(state, "\n");
	}

	let trimmed = caret.offset - 2;
	let Some(removed) = state.change(EditKind::Structure, |tr| {
		tr.delete_text(caret.block, trimmed..caret.offset)?;
		tr.set_selection(Selection::caret(Point::new(caret.block, trimmed)));
		Ok(true)
	})?
	else {
		return Ok(None);
	};
	let split = commands::split_block(state)?;

	Ok(Some(match split {
		Some(split) => removed.then(split),
		None => removed,
	}))
}

/// Backspace at the start of an empty code block turns it into a paragraph.
fn reset(state: &mut EditorState, kind: BlockTypeId) -> CommandResult {
	let Some(caret) = caret_in(state, kind) else {
		return Ok(None);
	};

	if state
		.doc()
		.inline(caret.block)
		.is_some_and(|text| !text.is_empty())
	{
		return Ok(None);
	}

	let paragraph = state.schema().default_text_block();
	commands::set_block_type(state, paragraph, &Attrs::new())
}

fn language(found: &RuleMatch) -> Attrs {
	let language = found.group(1).map_or(Value::Null, Value::from);
	Attrs::new().with("language", language)
}

impl Extension for CodeBlock {
	fn name(&self) -> &'static str {
		"codeBlock"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = Attrs::new().with("language", Value::Null);
		schema.block(BlockSpec::text("codeBlock").attrs(attrs))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, "codeBlock").attr(language_field())]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let toggle_spec = CommandSpec::new(
			"toggleCodeBlock",
			"Make the selected text blocks code, or paragraphs if they already are code.",
			|args| Ok(toggle(args.optional_string("language")?)),
		);
		let set_spec = CommandSpec::new(
			"setCodeBlock",
			"Make the selected text blocks code in a language.",
			|args| Ok(set(args.optional_string("language")?)),
		);
		Ok(vec![
			toggle_spec.arg(language_field()),
			set_spec.arg(language_field()),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let is_code = QuerySpec::new(
			"isCodeBlock",
			"Whether the selection is in a code block.",
			ValueType::Bool,
			|state, _| Ok(Value::Bool(resolve::around(state, "codeBlock").is_some())),
		);
		let language = QuerySpec::new(
			"codeBlockLanguage",
			"The language of the code block holding the selection, or null.",
			ValueType::nullable(ValueType::String),
			|state, _| Ok(language_at(state).map_or(Value::Null, Value::from)),
		);
		Ok(vec![is_code, language])
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		let kind = Self::kind(schema)?;
		Ok(vec![
			("Mod-Alt-c", toggle(None)),
			("Enter", command(move |state| enter(state, kind))),
			("Backspace", command(move |state| reset(state, kind))),
		])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = Self::kind(schema)?;
		Ok(vec![
			InputRule::block_type(r"^```([a-z]+)?[\s\n]$", kind, language)?,
			InputRule::block_type(r"^~~~([a-z]+)?[\s\n]$", kind, language)?,
		])
	}

	fn code_blocks(&self, schema: &Schema) -> Result<Vec<BlockTypeId>, EditorError> {
		Ok(vec![Self::kind(schema)?])
	}
}

impl TypedExtension for CodeBlock {
	type Handle = CodeBlockHandle;

	fn handle(&self, registration: Registration) -> CodeBlockHandle {
		CodeBlockHandle {
			_registration: registration,
		}
	}
}
