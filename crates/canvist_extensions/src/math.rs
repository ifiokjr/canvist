//! Mathematics: LaTeX formulas inline and as blocks.

use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::CommandSpec;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::InputRule;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Registration;
use canvist_editor::RuleMatch;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::BlockTypeId;
use canvist_model::Embed;
use canvist_model::EmbedSpec;
use canvist_model::EmbedTypeId;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Step;

use crate::resolve;

const INLINE: &str = "inlineMath";
const BLOCK: &str = "blockMath";

/// TipTap's Mathematics: an `inlineMath` embed and a `blockMath` block,
/// each holding LaTeX in `latex`.
///
/// Typing `$$x^2$$` inserts an inline formula, and `$$$x^2$$$` alone on a
/// line a block one. The canvas draws inline LaTeX as its source in a tinted
/// box and reserves a slot for block formulas, so the host can typeset them
/// with KaTeX or MathJax. HTML uses TipTap's `data-type` spans and divs;
/// Markdown writes `$…$` and `$$` blocks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mathematics;

/// Typed access to [`Mathematics`].
#[derive(Clone, Copy, Debug)]
pub struct MathematicsHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl MathematicsHandle {
	/// Insert an inline formula at the selection.
	#[must_use]
	pub fn insert_inline(self, latex: &str) -> Command {
		insert_inline(latex)
	}

	/// Insert a block formula at the selection.
	#[must_use]
	pub fn insert_block(self, latex: &str) -> Command {
		insert_block(latex)
	}
}

fn latex(latex: &str) -> Attrs {
	Attrs::new().with("latex", latex)
}

fn insert_inline(source: &str) -> Command {
	let attrs = latex(source);

	command(move |state| {
		let kind = resolve::embed(state, INLINE, INLINE)?;
		commands::insert_embed(
			state,
			Embed {
				kind,
				attrs: attrs.clone(),
			},
		)
	})
}

fn insert_block(source: &str) -> Command {
	let attrs = latex(source);

	command(move |state| {
		let kind = resolve::block(state, BLOCK, BLOCK)?;
		commands::insert_leaf(state, kind, attrs.clone())
	})
}

/// Replace typed `$$latex$$` with an inline formula.
fn typed_inline(state: &mut EditorState, found: &RuleMatch, kind: EmbedTypeId) -> CommandResult {
	let (Some(Some(source)), Some(text)) = (found.groups.get(1).cloned(), found.group(1)) else {
		return Ok(None);
	};
	let start = source.start - 2;
	let marks = commands::marks_at(state.doc(), Point::new(found.block, start));
	let content = InlineText::embed(
		Embed {
			kind,
			attrs: latex(text),
		},
		marks,
	);
	let end = Point::new(found.block, start + content.len());

	state.change(EditKind::InputRule, |tr| {
		tr.delete_text(found.block, start..found.caret)?;
		tr.step(Step::InsertInline {
			block: found.block,
			offset: start,
			content,
		})?;
		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

/// Replace a line typed as `$$$latex$$$` with a block formula.
fn typed_block(state: &mut EditorState, found: &RuleMatch, kind: BlockTypeId) -> CommandResult {
	let Some(text) = found.group(1) else {
		return Ok(None);
	};
	let attrs = latex(text);

	state.atomic(|state| {
		let cleared = state.change(EditKind::InputRule, |tr| {
			tr.delete_text(found.block, found.start..found.caret)?;
			tr.set_selection(Selection::caret(Point::new(found.block, found.start)));
			Ok(true)
		})?;
		let inserted = commands::insert_leaf(state, kind, attrs)?;

		Ok(cleared
			.zip(inserted)
			.map(|(cleared, inserted)| cleared.then(inserted)))
	})
}

fn latex_field(required: bool) -> Field {
	let doc = "The formula, in LaTeX.";

	if required {
		Field::required("latex", ValueType::String, doc)
	} else {
		Field::optional("latex", ValueType::String, "", doc)
	}
}

impl Extension for Mathematics {
	fn name(&self) -> &'static str {
		"mathematics"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema
			.embed(EmbedSpec::new(INLINE).attrs(latex("")))
			.block(BlockSpec::leaf(BLOCK).attrs(latex("")))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![
			NodeMeta::new(NodeKind::Embed, INLINE).attr(latex_field(false)),
			NodeMeta::new(NodeKind::Block, BLOCK).attr(latex_field(false)),
		]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let inline = CommandSpec::new(
			"insertInlineMath",
			"Insert an inline formula at the selection.",
			|args| Ok(insert_inline(args.string("latex")?)),
		);
		let block = CommandSpec::new(
			"insertBlockMath",
			"Insert a block formula at the selection.",
			|args| Ok(insert_block(args.string("latex")?)),
		);

		Ok(vec![
			inline.arg(latex_field(true)),
			block.arg(latex_field(true)),
		])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let inline = canvist_editor::embed_type(schema, INLINE, INLINE)?;
		let block = canvist_editor::block_type(schema, BLOCK, BLOCK)?;
		let block_rule = InputRule::new(r"^\$\$\$([^$]+)\$\$\$$", move |state, found| {
			typed_block(state, found, block)
		})?;
		let inline_rule = InputRule::new(r"(?:^|[^$])\$\$([^$\n]+)\$\$$", move |state, found| {
			typed_inline(state, found, inline)
		})?;

		Ok(vec![block_rule, inline_rule])
	}
}

impl TypedExtension for Mathematics {
	type Handle = MathematicsHandle;

	fn handle(&self, registration: Registration) -> MathematicsHandle {
		MathematicsHandle {
			_registration: registration,
		}
	}
}
