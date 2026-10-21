//! Placeholder text in empty blocks.

use std::sync::Arc;

use canvist_editor::Command;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::BlockId;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::Value;

/// The editor setting holding placeholder text set at run time.
const TEXT: &str = "placeholder";

/// TipTap's default placeholder.
const DEFAULT_TEXT: &str = "Write something …";

/// Hint text shown in empty blocks, such as "Write something …".
///
/// The placeholder is not part of the document. The extension says which
/// empty blocks get it, [`PlaceholderHandle::placeholders`], and hosts pass
/// them to `canvist_layout::DisplayOptions::placeholder`, which draws the
/// text in the block in a faded colour, the same on every platform.
#[derive(Clone, Copy, Debug)]
pub struct Placeholder {
	/// The text, until `setPlaceholder` changes it.
	pub text: &'static str,
	/// Only the empty block holding the caret shows it.
	pub show_only_current: bool,
	/// Empty blocks inside lists, quotes, and other containers show it too,
	/// not only top-level ones.
	pub include_children: bool,
}

impl Default for Placeholder {
	fn default() -> Self {
		Self {
			text: DEFAULT_TEXT,
			show_only_current: true,
			include_children: false,
		}
	}
}

/// Typed access to [`Placeholder`].
#[derive(Clone, Copy, Debug)]
pub struct PlaceholderHandle {
	config: Placeholder,
	_registration: Registration,
}

impl PlaceholderHandle {
	/// The empty blocks that show the placeholder, with its text.
	#[must_use]
	pub fn placeholders(self, state: &EditorState) -> Vec<(BlockId, Arc<str>)> {
		self.config.placeholders(state)
	}

	/// Change the placeholder text.
	#[allow(
		clippy::unused_self,
		reason = "handle methods require proof of registration"
	)]
	#[must_use]
	pub fn set(self, text: &str) -> Command {
		set(text)
	}
}

impl Placeholder {
	/// The text in force: one set at run time, or the configured text.
	fn text_in(&self, state: &EditorState) -> Arc<str> {
		match state.settings().get(TEXT).and_then(Value::as_str) {
			Some(text) => Arc::from(text),
			None => Arc::from(self.text),
		}
	}

	/// Whether `block` is an empty text block that may show a placeholder.
	fn eligible(&self, state: &EditorState, block: BlockId) -> bool {
		let doc = state.doc();
		let top_level = doc
			.block(block)
			.is_some_and(|block| block.parent().is_none());

		doc.inline(block)
			.is_some_and(canvist_model::InlineText::is_empty)
			&& (self.include_children || top_level)
	}

	fn placeholders(&self, state: &EditorState) -> Vec<(BlockId, Arc<str>)> {
		let text = self.text_in(state);
		let blocks: Vec<BlockId> = if self.show_only_current {
			current_block(state).into_iter().collect()
		} else {
			state
				.doc()
				.text_blocks()
				.map(canvist_model::Block::id)
				.collect()
		};

		blocks
			.into_iter()
			.filter(|&block| self.eligible(state, block))
			.map(|block| (block, text.clone()))
			.collect()
	}

	/// The placeholder the caret's block shows, if it shows one.
	fn current(&self, state: &EditorState) -> Option<Arc<str>> {
		let block = current_block(state)?;
		self.eligible(state, block).then(|| self.text_in(state))
	}
}

/// The text block holding a caret.
fn current_block(state: &EditorState) -> Option<BlockId> {
	match state.selection() {
		Selection::Text { anchor, head } if anchor == head => Some(head.block),
		_ => None,
	}
}

fn set(text: &str) -> Command {
	let text = Value::from(text);
	command(move |state| commands::set_setting(state, TEXT, text.clone()))
}

impl Extension for Placeholder {
	fn name(&self) -> &'static str {
		"placeholder"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let spec = CommandSpec::new("setPlaceholder", "Change the placeholder text.", |args| {
			Ok(set(args.string("placeholder")?))
		});
		let text = Field::required("placeholder", ValueType::String, "The placeholder text.");
		Ok(vec![spec.arg(text)])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let config = *self;

		Ok(vec![QuerySpec::new(
			"placeholder",
			"The placeholder the caret's empty block shows, or null.",
			ValueType::nullable(ValueType::String),
			move |state, _| {
				Ok(config
					.current(state)
					.map_or(Value::Null, |text| Value::from(&*text)))
			},
		)])
	}
}

impl TypedExtension for Placeholder {
	type Handle = PlaceholderHandle;

	fn handle(&self, registration: Registration) -> PlaceholderHandle {
		PlaceholderHandle {
			config: *self,
			_registration: registration,
		}
	}
}
