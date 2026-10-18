//! Code blocks.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
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
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

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

	fn toggle(kind: BlockTypeId) -> Command {
		command(move |state| commands::toggle_block_type(state, kind, &Attrs::new()))
	}
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

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		Ok(vec![("toggleCodeBlock", Self::toggle(Self::kind(schema)?))])
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		let kind = Self::kind(schema)?;
		Ok(vec![
			("Mod-Alt-c", Self::toggle(kind)),
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
