//! Character and word counts, and a character limit.

use canvist_editor::Command;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::Update;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Schema;
use canvist_model::Step;
use canvist_model::TreeContent;
use canvist_model::Value;
use unicode_segmentation::UnicodeSegmentation;

/// The editor setting holding a limit set at run time.
const LIMIT: &str = "characterLimit";

/// Counts characters and words, and with a `limit`, refuses edits that would
/// take the document past it, as TipTap's `CharacterCount` does.
///
/// A character is what a reader sees as one: a grapheme cluster, so `👍🏽`
/// and `é` written as `e` plus an accent each count once. (TipTap counts
/// UTF-16 code units by default.) Inline embeds such as mentions count as
/// none, and blocks are separate words. An edit that would go past the limit
/// does not apply, so a paste that would overflow is refused whole; edits
/// that shrink an over-long document always apply.
#[derive(Clone, Copy, Debug, Default)]
pub struct CharacterCount {
	/// The most characters the document may hold, until
	/// `setCharacterLimit` changes it.
	pub limit: Option<u32>,
}

/// Typed access to [`CharacterCount`].
#[derive(Clone, Copy, Debug)]
pub struct CharacterCountHandle {
	limit: Option<u32>,
	_registration: Registration,
}

impl CharacterCountHandle {
	/// The characters in the document.
	#[must_use]
	pub fn characters(self, state: &EditorState) -> usize {
		characters(state.doc())
	}

	/// The words in the document.
	#[must_use]
	pub fn words(self, state: &EditorState) -> usize {
		words(state.doc())
	}

	/// The limit in force, if there is one.
	#[must_use]
	pub fn limit(self, state: &EditorState) -> Option<u32> {
		limit_in(state, self.limit)
	}

	/// Change the limit, or remove it with `None`.
	#[must_use]
	pub fn set_limit(self, limit: Option<u32>) -> Command {
		set_limit(limit.map_or(Value::Null, |limit| Value::Int(i64::from(limit))))
	}
}

/// The grapheme clusters in a block's text, leaving out embeds.
fn text_characters(text: &InlineText) -> usize {
	text.runs()
		.filter(|run| run.embed.is_none())
		.map(|run| run.text.graphemes(true).count())
		.sum()
}

/// The words in a block's text, leaving out embeds.
fn text_words(text: &InlineText) -> usize {
	text.runs()
		.filter(|run| run.embed.is_none())
		.map(|run| run.text.split_whitespace().count())
		.sum()
}

fn characters(doc: &Document) -> usize {
	doc.text_blocks()
		.filter_map(canvist_model::Block::inline)
		.map(text_characters)
		.sum()
}

fn words(doc: &Document) -> usize {
	doc.text_blocks()
		.filter_map(canvist_model::Block::inline)
		.map(text_words)
		.sum()
}

/// The characters in every text block of `tree`.
fn tree_characters(tree: &BlockTree) -> usize {
	let mut count = 0;

	tree.walk(&mut |node: &BlockTree| {
		if let TreeContent::Inline(text) = &node.content {
			count += text_characters(text);
		}
	});

	count
}

/// The characters `steps` insert.
fn inserted(steps: &[Step]) -> usize {
	steps
		.iter()
		.map(|step| {
			match step {
				Step::InsertInline { content, .. } => text_characters(content),
				Step::InsertBlocks { blocks, .. } => blocks.iter().map(tree_characters).sum(),
				_ => 0,
			}
		})
		.sum()
}

/// The limit in force: one set at run time, or the configured default.
fn limit_in(state: &EditorState, configured: Option<u32>) -> Option<u32> {
	match state.settings().get(LIMIT) {
		Some(Value::Int(limit)) => u32::try_from(*limit).ok(),
		Some(_) => None,
		None => configured,
	}
}

/// A `limit` argument: null, or a count that fits `u32`.
fn limit_arg(limit: &Value) -> Result<Value, EditorError> {
	match limit {
		Value::Int(count) if u32::try_from(*count).is_err() => {
			Err(EditorError::InvalidArgs {
				name: "setCharacterLimit".to_owned(),
				reason: format!("a limit of {count} characters is out of range"),
			})
		}
		other => Ok(other.clone()),
	}
}

fn set_limit(limit: Value) -> Command {
	command(move |state| commands::set_setting(state, LIMIT, limit.clone()))
}

impl Extension for CharacterCount {
	fn name(&self) -> &'static str {
		"characterCount"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let spec = CommandSpec::new(
			"setCharacterLimit",
			"Change the character limit, or remove it with null.",
			|args| Ok(set_limit(limit_arg(args.get("limit")?)?)),
		);
		let limit = Field::required(
			"limit",
			ValueType::nullable(ValueType::Int),
			"The most characters the document may hold, from 0 to 4294967295, or null for no \
			 limit.",
		);
		Ok(vec![spec.arg(limit)])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let configured = self.limit;

		Ok(vec![
			QuerySpec::new(
				"characters",
				"The characters in the document: grapheme clusters, leaving out embeds.",
				ValueType::Int,
				|state, _| Ok(Value::Int(characters(state.doc()) as i64)),
			),
			QuerySpec::new(
				"words",
				"The words in the document.",
				ValueType::Int,
				|state, _| Ok(Value::Int(words(state.doc()) as i64)),
			),
			QuerySpec::new(
				"characterLimit",
				"The character limit in force, or null.",
				ValueType::nullable(ValueType::Int),
				move |state, _| {
					Ok(limit_in(state, configured)
						.map_or(Value::Null, |limit| Value::Int(i64::from(limit))))
				},
			),
		])
	}

	/// Refuse a local edit that adds characters past the limit. Counting
	/// the document is linear, but only runs for edits that add text while a
	/// limit is set, when the document is at most the limit long.
	fn allows(&self, state: &EditorState, update: &Update) -> bool {
		let Some(limit) = limit_in(state, self.limit) else {
			return true;
		};
		let added = inserted(&update.commit.steps);
		let removed = inserted(&update.commit.inverses);

		added <= removed || characters(state.doc()) <= limit as usize
	}
}

impl TypedExtension for CharacterCount {
	type Handle = CharacterCountHandle;

	fn handle(&self, registration: Registration) -> CharacterCountHandle {
		CharacterCountHandle {
			limit: self.limit,
			_registration: registration,
		}
	}
}
