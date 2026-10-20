//! Comments: ranges of text tied to discussion threads that live outside
//! the document.

use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::CommandSpec;
use canvist_editor::EditKind;
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
use canvist_model::Attrs;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::resolve;

const COMMENT: &str = "comment";
const THREADS: &str = "threads";

/// Comments, as TipTap's Comments and Remirror's annotations anchor them: a
/// `comment` mark whose `threads` attribute lists the ids of the threads on
/// that text. One mark holding a list lets comments overlap, as one text
/// run can only carry each mark once.
///
/// `setComment` adds a thread to the selected text, `unsetComment` removes
/// it everywhere, and `activeComments` lists the threads at the caret. The
/// threads themselves, their messages, authors, and resolution, belong to
/// the host. The theme tints commented text; HTML writes
/// `<span data-type="comment" data-threads="…">`, and Markdown drops the
/// mark.
#[derive(Clone, Copy, Debug, Default)]
pub struct Comments;

/// Typed access to [`Comments`].
#[derive(Clone, Copy, Debug)]
pub struct CommentsHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl CommentsHandle {
	/// Add thread `thread` to the selected text.
	#[must_use]
	pub fn set(self, thread: &str) -> Command {
		set(thread)
	}

	/// Remove thread `thread` from the whole document.
	#[must_use]
	pub fn unset(self, thread: &str) -> Command {
		unset(thread)
	}

	/// The threads on the text at the selection head.
	#[must_use]
	pub fn active(self, state: &EditorState) -> Vec<String> {
		active(state)
	}

	/// Every thread in the document, in the order it first appears, with
	/// the text it covers.
	#[must_use]
	pub fn threads(self, state: &EditorState) -> Vec<(String, String)> {
		threads(state)
	}
}

/// The thread ids in a comment mark's attributes.
fn ids(attrs: Option<&Attrs>) -> Vec<String> {
	attrs
		.and_then(|attrs| attrs.get(THREADS))
		.and_then(Value::as_array)
		.unwrap_or_default()
		.iter()
		.filter_map(Value::as_str)
		.map(str::to_owned)
		.collect()
}

fn with_ids(ids: &[String]) -> Option<Attrs> {
	if ids.is_empty() {
		return None;
	}

	let values: Vec<Value> = ids.iter().map(|id| Value::from(id.as_str())).collect();
	Some(Attrs::new().with(THREADS, values))
}

fn set(thread: &str) -> Command {
	let thread = thread.to_owned();

	command(move |state| {
		if state.selection().is_caret() {
			return Ok(None);
		}

		let kind = resolve::mark(state, COMMENT, COMMENT)?;
		commands::update_mark_attrs(state, kind, &|current| {
			let mut ids = ids(current);

			if !ids.contains(&thread) {
				ids.push(thread.clone());
			}

			with_ids(&ids)
		})
	})
}

fn unset(thread: &str) -> Command {
	let thread = thread.to_owned();

	command(move |state| remove_everywhere(state, &thread))
}

/// Remove `thread` from every comment mark in the document.
fn remove_everywhere(state: &mut EditorState, thread: &str) -> CommandResult {
	let kind = resolve::mark(state, COMMENT, COMMENT)?;
	let doc = state.doc();
	let mut edits = Vec::new();

	for block in doc.text_blocks() {
		let Some(text) = block.inline() else {
			continue;
		};
		let mut offset = 0;

		for run in text.runs() {
			let len = run.text.len() as u32;
			let range = offset..offset + len;
			offset += len;
			let current = run.marks.get(kind).map(|mark| &mark.attrs);
			let mut ids = ids(current);

			if ids.iter().any(|id| id == thread) {
				ids.retain(|id| id != thread);
				edits.push((block.id(), range, with_ids(&ids)));
			}
		}
	}

	if edits.is_empty() {
		return Ok(None);
	}

	state.change(EditKind::Formatting, |tr| {
		for (block, range, attrs) in edits {
			match attrs {
				Some(attrs) => tr.add_mark(block, range, Mark::with_attrs(kind, attrs))?,
				None => tr.remove_mark(block, range, kind)?,
			}
		}

		Ok(true)
	})
}

/// The threads on the character before the selection head, or the one
/// after it at a block's start.
fn active(state: &EditorState) -> Vec<String> {
	let Some(kind) = state.schema().mark_type(COMMENT) else {
		return Vec::new();
	};
	let canvist_model::Selection::Text { head, .. } = state.selection() else {
		return Vec::new();
	};
	let Some(text) = state.doc().inline(head.block) else {
		return Vec::new();
	};
	let source = text.as_str();
	let offset = (head.offset as usize).min(source.len());
	let before = source[..offset].chars().next_back();
	let range = match before {
		Some(character) => (offset - character.len_utf8())..offset,
		None => 0..source.chars().next().map_or(0, char::len_utf8),
	};
	let marks = text
		.mark_runs(range.start as u32..range.end as u32)
		.ok()
		.and_then(|runs| runs.into_iter().next())
		.map(|(_, marks)| marks);

	ids(marks
		.as_ref()
		.and_then(|marks| marks.get(kind))
		.map(|mark| &mark.attrs))
}

fn threads(state: &EditorState) -> Vec<(String, String)> {
	let Some(kind) = state.schema().mark_type(COMMENT) else {
		return Vec::new();
	};
	let mut found: Vec<(String, String)> = Vec::new();

	for block in state.doc().text_blocks() {
		let Some(text) = block.inline() else {
			continue;
		};

		for run in text.runs() {
			for id in ids(run.marks.get(kind).map(|mark| &mark.attrs)) {
				match found.iter_mut().find(|(known, _)| *known == id) {
					Some((_, covered)) => covered.push_str(run.text),
					None => found.push((id, run.text.to_owned())),
				}
			}
		}
	}

	found
}

fn thread_field() -> Field {
	Field::required(
		"threadId",
		ValueType::String,
		"The thread's id, from the host.",
	)
}

impl Extension for Comments {
	fn name(&self) -> &'static str {
		"comments"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(
			MarkSpec::new(COMMENT)
				.inclusive(false)
				.attrs(Attrs::new().with(THREADS, Vec::<Value>::new())),
		)
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![
			CommandSpec::new("setComment", "Add a thread to the selected text.", |args| {
				Ok(set(args.string("threadId")?))
			})
			.arg(thread_field()),
			CommandSpec::new(
				"unsetComment",
				"Remove a thread from the whole document.",
				|args| Ok(unset(args.string("threadId")?)),
			)
			.arg(thread_field()),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![
			QuerySpec::new(
				"activeComments",
				"The threads on the text at the selection head.",
				ValueType::list(ValueType::String),
				|state, _| {
					let ids: Vec<Value> = active(state).into_iter().map(Value::from).collect();
					Ok(Value::from(ids))
				},
			),
			QuerySpec::new(
				"comments",
				"Every thread in the document, in the order it first appears, with the text it \
				 covers.",
				ValueType::list(ValueType::Record(vec![
					Field::required("threadId", ValueType::String, "The thread's id."),
					Field::required("text", ValueType::String, "The text it covers."),
				])),
				|state, _| {
					let list: Vec<Value> = threads(state)
						.into_iter()
						.map(|(id, text)| {
							Value::from(Attrs::new().with("threadId", id).with("text", text))
						})
						.collect();
					Ok(Value::from(list))
				},
			),
		])
	}
}

impl TypedExtension for Comments {
	type Handle = CommentsHandle;

	fn handle(&self, registration: Registration) -> CommentsHandle {
		CommentsHandle {
			_registration: registration,
		}
	}
}
