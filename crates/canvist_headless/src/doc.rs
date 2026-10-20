//! A document with an editor around it, driven by explicit calls.

use std::time::Duration;

use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::Editor;
use canvist_editor::EditorState;
use canvist_editor::Env;
use canvist_editor::KeyPress;
use canvist_editor::LineNavigator;
use canvist_editor::Update;
use canvist_editor::commands;
use canvist_editor::parse_binding;
use canvist_editor::value_to_json;
use canvist_model::Attrs;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Value;
use serde_json::Value as Json;

use crate::Format;
use crate::HeadlessError;
use crate::Position;
use crate::Snapshot;
use crate::format;
use crate::position;

/// One document and the editor that changes it, with no layout and no UI.
///
/// Selections are explicit: set one with [`Doc::select`],
/// [`Doc::select_range`], or [`Doc::select_all`], then run commands. Every
/// call is one undo step. A `Doc` is `Send`, so it can move between threads
/// or live behind a `Mutex` in a server.
///
/// ```
/// use canvist_headless::Engine;
/// use canvist_headless::Format;
/// use canvist_headless::Position;
///
/// let engine = Engine::starter_kit();
/// let kit = engine.handles();
/// let mut doc = engine.parse(Format::Markdown, "Hello world")?;
///
/// doc.select_range(&Position::new([0], 6), &Position::new([0], 11))?;
/// doc.run(&kit.italic.toggle())?;
///
/// assert_eq!(doc.export(Format::Markdown), "Hello *world*");
/// # Ok::<(), canvist_headless::HeadlessError>(())
/// ```
pub struct Doc {
	pub(crate) editor: Editor,
	/// The time of the last call. Calls without a host clock advance it by
	/// one second, so each is its own undo step and results never depend on
	/// the wall clock; [`Doc::edit`] moves it to the host's time.
	clock: Duration,
	/// Every commit that changed the document since the host last took
	/// them, while [`Doc::record_commits`] is on.
	journal: Option<Vec<Commit>>,
	#[cfg(feature = "collab")]
	pub(crate) shared: Option<canvist_collab::Binding>,
}

impl std::fmt::Debug for Doc {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Doc")
			.field("editor", &self.editor)
			.finish_non_exhaustive()
	}
}

impl Doc {
	pub(crate) fn new(editor: Editor) -> Self {
		Self {
			editor,
			clock: Duration::ZERO,
			journal: None,
			#[cfg(feature = "collab")]
			shared: None,
		}
	}

	/// The editor, for typed queries such as
	/// `kit.bold.is_active(doc.editor().state())`.
	#[must_use]
	pub fn editor(&self) -> &Editor {
		&self.editor
	}

	/// The editor state: document, selection, and stored marks.
	#[must_use]
	pub fn state(&self) -> &EditorState {
		self.editor.state()
	}

	/// The model document.
	#[must_use]
	pub fn document(&self) -> &Document {
		self.editor.state().doc()
	}

	/// The document as `format`.
	#[must_use]
	pub fn export(&self, format: Format) -> String {
		format::write(self.document(), format)
	}

	/// The document as ProseMirror and TipTap JSON, with block ids in
	/// `attrs.id`.
	#[must_use]
	pub fn to_json(&self) -> Json {
		self.document().to_json()
	}

	/// The content and selection, to [`Engine::restore`](crate::Engine::restore)
	/// later or elsewhere.
	#[must_use]
	pub fn snapshot(&self) -> Snapshot {
		Snapshot::new(self.to_json(), self.selected_range())
	}

	/// The selection.
	#[must_use]
	pub fn selection(&self) -> Selection {
		self.editor.state().selection()
	}

	/// The anchor and head of a text selection or caret as [`Position`]s,
	/// or `None` for a block selection.
	#[must_use]
	pub fn selected_range(&self) -> Option<(Position, Position)> {
		let Selection::Text { anchor, head } = self.selection() else {
			return None;
		};

		Some((self.position(anchor)?, self.position(head)?))
	}

	/// The model point `position` names.
	pub fn point(&self, position: &Position) -> Result<Point, HeadlessError> {
		position::resolve(self.document(), position)
	}

	/// The position of a model point, or `None` when it is not in this
	/// document.
	#[must_use]
	pub fn position(&self, point: Point) -> Option<Position> {
		position::locate(self.document(), point)
	}

	/// Replace the selection. Points outside the document are repaired to
	/// the nearest valid ones.
	pub fn select(&mut self, selection: Selection) -> Result<(), HeadlessError> {
		let mut env = self.env();
		let update = self.editor.select(selection, &mut env)?;
		self.finish(update.as_ref()).map(drop)
	}

	/// Select from `anchor` to `head`; equal positions make a caret.
	pub fn select_range(
		&mut self,
		anchor: &Position,
		head: &Position,
	) -> Result<(), HeadlessError> {
		let anchor = self.point(anchor)?;
		let head = self.point(head)?;
		self.select(Selection::text(anchor, head))
	}

	/// Select the whole document.
	pub fn select_all(&mut self) -> Result<(), HeadlessError> {
		self.run(&canvist_editor::command(commands::select_all))
			.map(drop)
	}

	/// Run `command`, usually from a typed handle such as
	/// `kit.bold.toggle()`. Returns whether it applied.
	pub fn run(&mut self, command: &Command) -> Result<bool, HeadlessError> {
		let mut env = self.env();
		let update = self.editor.run(command, &mut env)?;
		self.finish(update.as_ref())
	}

	/// Whether `command` would apply. Nothing changes.
	pub fn can(&mut self, command: &Command) -> Result<bool, HeadlessError> {
		let mut env = self.env();
		Ok(self.editor.can(command, &mut env)?)
	}

	/// Run `commands` as one change and one undo step, or not at all: if
	/// any does not apply, everything rolls back. Returns whether they
	/// applied.
	pub fn chain(&mut self, commands: &[Command]) -> Result<bool, HeadlessError> {
		let mut env = self.env();
		let chain = commands.iter().fold(self.editor.chain(), |chain, command| {
			chain.then(command.clone())
		});
		let update = chain.run(&mut env)?;
		self.finish(update.as_ref())
	}

	/// Run the command an extension registered as `name`, such as
	/// `toggleBold` or `setHeading` with `{ "level": 2 }`. `args` is a JSON
	/// object, or `null` for none. This is the path for bindings and
	/// scripts; Rust code uses typed handles.
	pub fn run_named(&mut self, name: &str, args: &Json) -> Result<bool, HeadlessError> {
		let command = self.command_named(name, args)?;
		self.run(&command)
	}

	/// The command an extension registered as `name`, with `args` (a JSON
	/// object, or `null` for none) checked against its metadata, for hosts
	/// that run it through [`Doc::edit`].
	pub fn command_named(&self, name: &str, args: &Json) -> Result<Command, HeadlessError> {
		Ok(self.editor.command_named(name, &attrs(name, args)?)?)
	}

	/// Whether the named command would apply. Nothing changes.
	pub fn can_named(&mut self, name: &str, args: &Json) -> Result<bool, HeadlessError> {
		let command = self.command_named(name, args)?;
		self.can(&command)
	}

	/// Answer the query an extension registered as `name`, such as `isBold`
	/// or `headingLevel`.
	pub fn query(&self, name: &str, args: &Json) -> Result<Json, HeadlessError> {
		let value = self.editor.query(name, &attrs(name, args)?)?;
		Ok(value_to_json(&value))
	}

	/// Insert `text` at the selection, replacing it, without running input
	/// rules: `"## "` stays text. Inserting `""` deletes the selection.
	pub fn insert_text(&mut self, text: &str) -> Result<bool, HeadlessError> {
		if text.is_empty() {
			return self.delete_selection();
		}

		let text = text.to_owned();
		let command = canvist_editor::command(move |state| commands::insert_text(state, &text));
		self.run(&command)
	}

	/// Delete the selected text or blocks, joining the blocks at the ends of
	/// a text range. Returns whether there was a selection to delete.
	pub fn delete_selection(&mut self) -> Result<bool, HeadlessError> {
		self.run(&canvist_editor::command(commands::delete_selection))
	}

	/// Type `text` one character at a time, as a keyboard would, so input
	/// rules apply: `"## "` turns the block into a heading. Returns whether
	/// anything changed.
	pub fn type_text(&mut self, text: &str) -> Result<bool, HeadlessError> {
		let mut changed = false;
		let mut buffer = [0; 4];

		for character in text.chars() {
			let mut env = self.env();
			let update = self
				.editor
				.insert_text(character.encode_utf8(&mut buffer), &mut env)?;
			changed |= self.finish(update.as_ref())?;
		}

		Ok(changed)
	}

	/// Press a key or chord, written as key bindings are: `Enter`,
	/// `Backspace`, `Mod-b`, `Shift-Tab`. `Mod` is Ctrl. Returns whether a
	/// bound command applied.
	pub fn press(&mut self, key: &str) -> Result<bool, HeadlessError> {
		let (key, modifiers) = parse_binding(key, self.editor.platform())?;
		let mut env = self.env();
		let update = self
			.editor
			.handle_key(KeyPress::new(&key, modifiers), &mut env)?;
		self.finish(update.as_ref())
	}

	/// Paste `content` read as `format` over the selection, as a user
	/// pasting would: paste rules run, and text pasted into code stays
	/// literal.
	pub fn paste(&mut self, format: Format, content: &str) -> Result<bool, HeadlessError> {
		let mut env = self.env();
		let update = match format {
			Format::Text => self.editor.paste_text(content, &mut env)?,
			format => {
				let slice = format::read_slice(self.document().schema(), format, content)?;
				self.editor.paste(slice, &mut env)?
			}
		};
		self.finish(update.as_ref())
	}

	/// Undo the last call that changed the document. Returns whether there
	/// was anything to undo.
	pub fn undo(&mut self) -> Result<bool, HeadlessError> {
		#[cfg(feature = "collab")]
		if self.shared.is_some() {
			return self.undo_shared(false);
		}

		let mut env = self.env();
		let update = self.editor.undo(&mut env)?;
		self.finish(update.as_ref())
	}

	/// Redo the last undone call. Returns whether there was anything to
	/// redo.
	pub fn redo(&mut self) -> Result<bool, HeadlessError> {
		#[cfg(feature = "collab")]
		if self.shared.is_some() {
			return self.undo_shared(true);
		}

		let mut env = self.env();
		let update = self.editor.redo(&mut env)?;
		self.finish(update.as_ref())
	}

	/// Change the document through the editor with an interactive host's
	/// clock and line navigator, and return the update so the host can lay
	/// out again.
	///
	/// Keys such as `ArrowUp` and `End` then move by visual lines, and typing
	/// groups into undo steps by real time. Like every other call, the update
	/// passes on to collaborators. `now` never runs backwards: a time before
	/// the last call's counts as that time.
	pub fn edit(
		&mut self,
		now: Duration,
		lines: &mut dyn LineNavigator,
		change: impl FnOnce(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> Result<Option<Update>, HeadlessError> {
		self.clock = self.clock.max(now);
		let mut env = Env::new(self.clock, lines);
		let update = change(&mut self.editor, &mut env)?;
		self.finish(update.as_ref())?;

		Ok(update)
	}

	/// Record every commit that changes the document from now on, for a host
	/// that keeps a layout of it: local calls, collaborators' updates, the
	/// shared history, and the repairs collaboration makes. Take them with
	/// [`Doc::take_commits`] and pass each to the layout in order.
	///
	/// Hosts that make every change through [`Doc::edit`] can apply the
	/// updates it returns instead, but a host that also lets other code run
	/// commands, or collaborates, sees only some changes that way.
	pub fn record_commits(&mut self) {
		self.journal.get_or_insert_with(Vec::new);
	}

	/// The commits recorded since the last call, oldest first. Empty unless
	/// [`Doc::record_commits`] is on.
	pub fn take_commits(&mut self) -> Vec<Commit> {
		self.journal
			.as_mut()
			.map(std::mem::take)
			.unwrap_or_default()
	}

	/// Note `update` in the journal when it changed the document.
	pub(crate) fn record(&mut self, update: &Update) {
		if let Some(journal) = &mut self.journal
			&& update.doc_changed()
		{
			journal.push(update.commit.clone());
		}
	}

	/// A fresh environment, one logical second after the last call.
	pub(crate) fn env(&mut self) -> Env<'static> {
		self.clock += Duration::from_secs(1);
		Env::headless(self.clock)
	}

	/// Pass a local update on to collaborators, and report whether there
	/// was one.
	#[cfg_attr(
		not(feature = "collab"),
		allow(
			clippy::unnecessary_wraps,
			reason = "only shared documents pass updates on"
		)
	)]
	fn finish(&mut self, update: Option<&Update>) -> Result<bool, HeadlessError> {
		let Some(update) = update else {
			return Ok(false);
		};

		self.record(update);
		#[cfg(feature = "collab")]
		self.mirror_local(update)?;

		Ok(true)
	}
}

/// Command and query arguments from a JSON object.
fn attrs(name: &str, args: &Json) -> Result<Attrs, HeadlessError> {
	let invalid = |reason: &str| {
		HeadlessError::InvalidArgs {
			name: name.to_owned(),
			reason: reason.to_owned(),
		}
	};

	let fields = match args {
		Json::Null => return Ok(Attrs::new()),
		Json::Object(fields) => fields,
		_ => return Err(invalid("arguments must be an object")),
	};

	fields
		.iter()
		.map(|(key, value)| {
			value_of(value)
				.map(|value| (key.as_str(), value))
				.ok_or_else(|| {
					invalid(&format!(
						"`{key}` must be a string, number, boolean, null, or array"
					))
				})
		})
		.collect()
}

fn value_of(json: &Json) -> Option<Value> {
	Some(match json {
		Json::Null => Value::Null,
		Json::Bool(value) => Value::Bool(*value),
		Json::Number(number) => {
			match number.as_i64() {
				Some(int) => Value::from(int),
				None => Value::from(number.as_f64()?),
			}
		}
		Json::String(value) => Value::from(value.as_str()),
		Json::Array(values) => {
			Value::from(values.iter().map(value_of).collect::<Option<Vec<_>>>()?)
		}
		Json::Object(_) => return None,
	})
}
