//! Hashtags: `#canvas`, marked as you type.

use std::collections::BTreeSet;

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::CommandSpec;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::InputRule;
use canvist_editor::Lookback;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::PasteRule;
use canvist_editor::Platform;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::RuleMatch;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::mark_type;
use canvist_model::Attrs;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::MarkSpec;
use canvist_model::MarkTypeId;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

use crate::suggestion;
use crate::suggestion::Trigger;

/// The mark's name.
const HASHTAG: &str = "hashtag";

/// A tag: `#` and then anything but white space, `#`, and punctuation that
/// ends a word.
const TAG: &str = r#"#[^\s#.,;:!?()\[\]{}"'<>]+"#;

/// What can follow a tag as it is typed.
const AFTER: &str = r"[\s.,;:!?)\]]";

/// The characters [`AFTER`] matches.
const AFTER_CHARS: &str = " \t\n.,;:!?)]";

/// A `hashtag` mark over `#tag` text, with the tag (without `#`) in its
/// `tag` attribute. Neither TipTap nor Remirror has one.
///
/// A tag is marked when the space or punctuation after it is typed, and in
/// pasted text. Typing `#` starts a suggestion, reported by the
/// `hashtagSuggestion` query, and `insertHashtag` completes it. The
/// `hashtags` query lists every tag in the document, for a tag cloud or a
/// search filter.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hashtag;

/// Typed access to [`Hashtag`].
#[derive(Clone, Copy, Debug)]
pub struct HashtagHandle {
	_registration: Registration,
}

const TRIGGER: Trigger = Trigger {
	name: HASHTAG,
	char: '#',
	allow_spaces: false,
};

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HashtagHandle {
	/// The query typed after `#`, if a hashtag is being suggested.
	#[must_use]
	pub fn suggestion(self, state: &EditorState) -> Option<String> {
		TRIGGER.find(state).map(|found| found.query)
	}

	/// Replace the suggestion, or insert at the caret, with `#tag` and a
	/// space.
	#[must_use]
	pub fn insert(self, tag: &str) -> Command {
		insert(tag)
	}

	/// Stop suggesting until another `#` is typed.
	#[must_use]
	pub fn dismiss(self) -> Command {
		dismiss()
	}

	/// Every tag in the document, sorted, without `#`.
	#[must_use]
	pub fn tags(self, state: &EditorState) -> Vec<String> {
		tags(state)
	}
}

fn hashtag(kind: MarkTypeId, tag: &str) -> Mark {
	Mark::with_attrs(kind, Attrs::new().with("tag", tag))
}

fn insert(tag: &str) -> Command {
	let tag = tag.trim_start_matches('#').to_owned();

	command(move |state| {
		let kind = crate::resolve::mark(state, HASHTAG, HASHTAG)?;
		let marks = commands::active_marks(state).with(&hashtag(kind, &tag), state.schema());
		let text = InlineText::plain(&format!("#{tag}"), marks);
		let found = TRIGGER.find(state);
		suggestion::replace(state, found, text)
	})
}

fn dismiss() -> Command {
	command(|state| TRIGGER.dismiss(state))
}

fn tags(state: &EditorState) -> Vec<String> {
	let Some(kind) = state.schema().mark_type(HASHTAG) else {
		return Vec::new();
	};
	let doc = state.doc();
	let tags: BTreeSet<String> = doc
		.text_blocks()
		.filter_map(canvist_model::Block::inline)
		.flat_map(InlineText::runs)
		.filter_map(|run| {
			run.marks
				.get(kind)?
				.attrs
				.get("tag")?
				.as_str()
				.map(str::to_owned)
		})
		.collect();

	tags.into_iter().collect()
}

/// Mark the tag before the caret, then type what ended it unmarked.
fn mark_typed(state: &mut EditorState, found: &RuleMatch, kind: MarkTypeId) -> CommandResult {
	let (Some(Some(range)), Some(tag)) = (found.groups.get(1).cloned(), found.group(1)) else {
		return Ok(None);
	};
	let mark = hashtag(kind, &tag[1..]);
	let caret = Point::new(found.block, found.caret);
	let marks: MarkSet = commands::marks_at(state.doc(), caret).without(kind);

	state.change(EditKind::InputRule, |tr| {
		tr.add_mark(found.block, range, mark)?;
		let end = tr.insert_text(caret, &found.typed, marks)?;
		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

impl Extension for Hashtag {
	fn name(&self) -> &'static str {
		HASHTAG
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = Attrs::new().with("tag", Value::Null);
		schema.mark(MarkSpec::new(HASHTAG).inclusive(false).attrs(attrs))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let tag = Field::optional(
			"tag",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"The tag, without `#`.",
		);
		vec![NodeMeta::new(NodeKind::Mark, HASHTAG).attr(tag)]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let insert_spec = CommandSpec::new(
			"insertHashtag",
			"Replace the hashtag suggestion, or insert at the caret, with `#tag` and a space.",
			|args| Ok(insert(args.string("tag")?)),
		)
		.arg(Field::required(
			"tag",
			ValueType::String,
			"The tag, with or without `#`.",
		));

		Ok(vec![
			insert_spec,
			CommandSpec::simple(
				"dismissHashtagSuggestion",
				"Stop suggesting hashtags until another `#` is typed.",
				dismiss(),
			),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![
			QuerySpec::new(
				"hashtagSuggestion",
				"The trigger character and the query typed after it, while a hashtag is suggested.",
				suggestion::query_type(),
				|state, _| Ok(TRIGGER.query_value(state)),
			),
			QuerySpec::new(
				"hashtags",
				"Every tag in the document, sorted, without `#`.",
				ValueType::list(ValueType::String),
				|state, _| {
					Ok(Value::from(
						tags(state).into_iter().map(Value::from).collect::<Vec<_>>(),
					))
				},
			),
		])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Escape", dismiss())])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = mark_type(schema, HASHTAG, HASHTAG)?;
		let pattern = format!("(?:^|\\s)({TAG}){AFTER}$");
		let rule = InputRule::new(&pattern, move |state, found| mark_typed(state, found, kind))?;
		Ok(vec![rule.on(AFTER_CHARS).lookback(Lookback::Word)])
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		let kind = mark_type(schema, HASHTAG, HASHTAG)?;
		let pattern = format!("(?:^|\\s)({TAG})");
		Ok(vec![PasteRule::mark_with(&pattern, kind, |tag| {
			Attrs::new().with("tag", tag.trim_start_matches('#'))
		})?])
	}
}

impl TypedExtension for Hashtag {
	type Handle = HashtagHandle;

	fn handle(&self, registration: Registration) -> HashtagHandle {
		HashtagHandle {
			_registration: registration,
		}
	}
}
