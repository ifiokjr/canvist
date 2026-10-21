//! Mentions: `@Ada`, an atomic inline embed with an id and a label.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::CommandSpec;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Platform;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_model::Attrs;
use canvist_model::Embed;
use canvist_model::EmbedSpec;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

use crate::suggestion;
use crate::suggestion::Trigger;

/// The embed's name, as in TipTap.
const MENTION: &str = "mention";

/// TipTap's `mention` node: an atom with `id`, `label`, and the
/// `mentionSuggestionChar` that started it.
///
/// Typing `@` starts a suggestion: the `mentionSuggestion` query reports
/// what follows it, the host shows matching people, and `insertMention`
/// replaces `@query` with the mention and a space. Escape dismisses the
/// suggestion. Backspace right after a mention turns it back into its
/// trigger character, so the suggestion opens again, as TipTap does by
/// default.
#[derive(Clone, Copy, Debug)]
pub struct Mention {
	/// The character that starts a mention.
	pub char: char,
	/// Whether the query may hold spaces, as `@Ada Lovelace` does.
	pub allow_spaces: bool,
}

impl Default for Mention {
	fn default() -> Self {
		Self {
			char: '@',
			allow_spaces: false,
		}
	}
}

/// A person, team, or anything else a mention points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MentionTarget<'a> {
	/// A stable id, such as a user id.
	pub id: &'a str,
	/// The text shown after the trigger, such as a name.
	pub label: Option<&'a str>,
}

/// Typed access to [`Mention`].
#[derive(Clone, Copy, Debug)]
pub struct MentionHandle {
	config: Mention,
	_registration: Registration,
}

impl MentionHandle {
	/// The query typed after the trigger, if a mention is being suggested.
	#[must_use]
	pub fn suggestion(self, state: &EditorState) -> Option<String> {
		self.config.trigger().find(state).map(|found| found.query)
	}

	/// Replace the suggestion, or insert at the caret, with a mention of
	/// `target` followed by a space.
	#[must_use]
	pub fn insert(self, target: MentionTarget<'_>) -> Command {
		self.config.insert(target.id, target.label)
	}

	/// Stop suggesting until another `@` is typed.
	#[must_use]
	pub fn dismiss(self) -> Command {
		self.config.dismiss()
	}
}

impl Mention {
	const fn trigger(self) -> Trigger {
		Trigger {
			name: MENTION,
			char: self.char,
			allow_spaces: self.allow_spaces,
		}
	}

	fn insert(self, id: &str, label: Option<&str>) -> Command {
		let attrs = Attrs::new()
			.with("id", id)
			.with("label", label.map_or(Value::Null, Value::from))
			.with("mentionSuggestionChar", self.char.to_string());

		command(move |state| {
			let kind = crate::resolve::embed(state, MENTION, MENTION)?;
			let marks = canvist_editor::commands::active_marks(state);
			let embed = InlineText::embed(
				Embed {
					kind,
					attrs: attrs.clone(),
				},
				marks,
			);
			let found = self.trigger().find(state);
			suggestion::replace(state, found, embed)
		})
	}

	fn dismiss(self) -> Command {
		command(move |state| self.trigger().dismiss(state))
	}

	/// Backspace right after a mention: replace it with its trigger
	/// character, opening the suggestion again.
	fn backspace(self) -> Command {
		command(move |state| reopen(state, self.char))
	}
}

/// Replace the mention just before the caret with the character that
/// started it.
fn reopen(state: &mut EditorState, default_char: char) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let Some(kind) = state.schema().embed_type(MENTION) else {
		return Ok(None);
	};
	let Some(text) = state.doc().inline(caret.block) else {
		return Ok(None);
	};
	let mention = text
		.runs()
		.find(|run| run.range.end == caret.offset)
		.and_then(|run| {
			let embed = run.embed.filter(|embed| embed.kind == kind)?;
			Some((run.range.start, embed.clone()))
		});
	let Some((start, mention)) = mention else {
		return Ok(None);
	};
	let trigger = mention
		.attrs
		.get("mentionSuggestionChar")
		.and_then(Value::as_str)
		.map_or_else(|| default_char.to_string(), str::to_owned);
	state.change(EditKind::Deleting, |tr| {
		tr.delete_text(caret.block, start..caret.offset)?;
		let end = tr.insert_text(Point::new(caret.block, start), &trigger, MarkSet::empty())?;
		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

fn fields() -> [Field; 3] {
	[
		Field::optional(
			"id",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"What the mention points at, such as a user id.",
		),
		Field::optional(
			"label",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"The text shown after the trigger character.",
		),
		Field::optional(
			"mentionSuggestionChar",
			ValueType::String,
			"@",
			"The character that started the mention.",
		),
	]
}

impl Extension for Mention {
	fn name(&self) -> &'static str {
		MENTION
	}

	/// Above the default, so Backspace after a mention reopens it before
	/// the standard keys delete it, as TipTap's priority 101 does.
	fn priority(&self) -> i32 {
		101
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = fields()
			.into_iter()
			.map(|field| (field.name, field.default.unwrap_or_default()))
			.collect();
		schema.embed(EmbedSpec::new(MENTION).attrs(attrs))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let node = NodeMeta::new(NodeKind::Embed, MENTION);
		vec![fields().into_iter().fold(node, NodeMeta::attr)]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let config = *self;
		let insert = CommandSpec::new(
			"insertMention",
			"Replace the mention suggestion, or insert at the caret, with a mention and a space.",
			move |args| Ok(config.insert(args.string("id")?, args.optional_string("label")?)),
		)
		.arg(Field::required(
			"id",
			ValueType::String,
			"What the mention points at.",
		))
		.arg(Field::optional(
			"label",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"The text shown after the trigger character.",
		));

		Ok(vec![
			insert,
			CommandSpec::simple(
				"dismissMentionSuggestion",
				"Stop suggesting mentions until another trigger character is typed.",
				self.dismiss(),
			),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let trigger = self.trigger();

		Ok(vec![QuerySpec::new(
			"mentionSuggestion",
			"The trigger character and the query typed after it, while a mention is suggested.",
			suggestion::query_type(),
			move |state, _| Ok(trigger.query_value(state)),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![
			("Escape", self.dismiss()),
			("Backspace", self.backspace()),
		])
	}
}

impl TypedExtension for Mention {
	type Handle = MentionHandle;

	fn handle(&self, registration: Registration) -> MentionHandle {
		MentionHandle {
			config: *self,
			_registration: registration,
		}
	}
}
