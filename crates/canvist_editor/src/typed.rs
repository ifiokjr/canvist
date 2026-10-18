//! The headless API: typed handles, named commands and queries, dry runs,
//! and chains.
//!
//! canvist ships no toolbar or menus. Everything an extension adds is
//! reachable through explicit calls, so an app builds its own UI on top:
//!
//! - **Typed handles.** [`EditorBuilder::add`] returns the extension's handle,
//!   whose methods are the only typed way to reach its commands and queries.
//!   Code cannot call bold's commands without having registered bold.
//! - **Named commands and queries** with checked arguments, for dynamic
//!   hosts and generated bindings ([`Editor::run_named`], [`Editor::query`]).
//! - **Metadata** describing all of it ([`Editor::metadata`]).
//! - **Dry runs** ([`Editor::can`]) and **chains** ([`Editor::chain`]) that
//!   change nothing unless every command applies.

use canvist_model::Attrs;
use canvist_model::Value;
use serde_json::Value as Json;

use crate::Command;
use crate::CommandResult;
use crate::Context;
use crate::EditKind;
use crate::Editor;
use crate::EditorBuilder;
use crate::EditorError;
use crate::EditorState;
use crate::Env;
use crate::Extension;
use crate::ExtensionMeta;
use crate::History;
use crate::Update;

/// Proof that an extension was registered with an [`EditorBuilder`]. Only
/// [`EditorBuilder::add`] creates one, so a handle holding it cannot exist
/// for an extension the app never added:
///
/// ```compile_fail
/// let forged = canvist_editor::Registration { _private: () };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Registration {
	_private: (),
}

/// An extension with a typed handle.
pub trait TypedExtension: Extension + Sized + 'static {
	/// The handle [`EditorBuilder::add`] returns: methods for this
	/// extension's commands, queries, and attributes.
	type Handle: Copy;

	/// The handle, given proof of registration.
	fn handle(&self, registration: Registration) -> Self::Handle;
}

impl EditorBuilder {
	/// Register `extension` and return its typed handle.
	///
	/// ```
	/// use canvist_editor::Editor;
	/// use canvist_editor::Extension;
	/// use canvist_editor::Registration;
	/// use canvist_editor::TypedExtension;
	/// use canvist_model::BlockSpec;
	/// use canvist_model::SchemaBuilder;
	///
	/// struct Paragraphs;
	///
	/// impl Extension for Paragraphs {
	/// 	fn name(&self) -> &'static str {
	/// 		"paragraphs"
	/// 	}
	///
	/// 	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
	/// 		schema.block(BlockSpec::text("paragraph"))
	/// 	}
	/// }
	///
	/// impl TypedExtension for Paragraphs {
	/// 	type Handle = Registration;
	///
	/// 	fn handle(&self, registration: Registration) -> Registration {
	/// 		registration
	/// 	}
	/// }
	///
	/// let mut builder = Editor::builder();
	/// let _paragraphs = builder.add(Paragraphs);
	/// let editor = builder.build()?;
	/// # let _ = editor;
	/// # Ok::<(), canvist_editor::EditorError>(())
	/// ```
	pub fn add<E: TypedExtension>(&mut self, extension: E) -> E::Handle {
		let handle = extension.handle(Registration { _private: () });
		self.extensions.push(std::sync::Arc::new(extension));
		handle
	}
}

/// History for dry runs and chains: reports what could be undone but never
/// changes the real history, and refuses to undo inside a chain.
struct DetachedHistory {
	can_undo: bool,
	can_redo: bool,
	/// Whether undo and redo pretend to apply (dry runs) or refuse (chains).
	pretend: bool,
}

impl DetachedHistory {
	fn answer(&self, possible: bool, state: &mut EditorState) -> CommandResult {
		if !(self.pretend && possible) {
			return Ok(None);
		}

		state.change(EditKind::Selection, |_| Ok(true))
	}
}

impl History for DetachedHistory {
	fn record(&mut self, _update: &Update, _now: std::time::Duration) {}

	fn undo(&mut self, state: &mut EditorState) -> CommandResult {
		self.answer(self.can_undo, state)
	}

	fn redo(&mut self, state: &mut EditorState) -> CommandResult {
		self.answer(self.can_redo, state)
	}

	fn can_undo(&self) -> bool {
		self.can_undo
	}

	fn can_redo(&self) -> bool {
		self.can_redo
	}

	fn close_group(&mut self) {}
}

impl Editor {
	fn detached_history(&self, pretend: bool) -> DetachedHistory {
		DetachedHistory {
			can_undo: self.history.can_undo(),
			can_redo: self.history.can_redo(),
			pretend,
		}
	}

	/// Whether `command` would apply right now. Nothing changes: not the
	/// document, its version or block revisions, the selection, or history.
	pub fn can(&mut self, command: &Command, env: &Env<'_>) -> Result<bool, EditorError> {
		let mut history = self.detached_history(true);
		let last_rule = self.last_rule.as_ref();

		self.state.dry_run(|state| {
			command(&mut Context {
				state,
				history: &mut history,
				lines: env.lines,
				last_rule,
			})
		})
	}

	/// Start a chain of commands that run as one update and one undo step.
	pub fn chain(&mut self) -> Chain<'_> {
		Chain {
			editor: self,
			commands: Vec::new(),
		}
	}

	/// Build the command an extension registered as `name`, checking
	/// `args` against its declared arguments.
	pub fn command_named(&self, name: &str, args: &Attrs) -> Result<Command, EditorError> {
		let spec = self
			.registry
			.commands
			.get(name)
			.ok_or_else(|| EditorError::UnknownName(name.to_owned()))?;
		spec.build(args)
	}

	/// Run the command an extension registered as `name` with `args`. This
	/// is the runtime-checked path for dynamic hosts; Rust code uses the
	/// typed handles instead.
	pub fn run_named(&mut self, name: &str, args: &Attrs, env: &Env<'_>) -> CommandResult {
		let command = self.command_named(name, args)?;
		self.run(&command, env)
	}

	/// Answer the query an extension registered as `name`.
	pub fn query(&self, name: &str, args: &Attrs) -> Result<Value, EditorError> {
		let spec = self
			.registry
			.queries
			.get(name)
			.ok_or_else(|| EditorError::UnknownName(name.to_owned()))?;
		spec.run(&self.state, args)
	}

	/// What every registered extension adds, in registration order.
	#[must_use]
	pub fn metadata(&self) -> &[ExtensionMeta] {
		&self.registry.metadata
	}

	/// [`Editor::metadata`] as JSON, the input for binding generators.
	#[must_use]
	pub fn metadata_json(&self) -> Json {
		Json::Array(
			self.registry
				.metadata
				.iter()
				.map(ExtensionMeta::to_json)
				.collect(),
		)
	}
}

/// Commands that run together as one update and one undo step, or not at
/// all: if any command does not apply or fails, everything rolls back.
/// Undo and redo cannot be part of a chain.
pub struct Chain<'e> {
	editor: &'e mut Editor,
	commands: Vec<Command>,
}

impl std::fmt::Debug for Chain<'_> {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Chain")
			.field("commands", &self.commands.len())
			.finish_non_exhaustive()
	}
}

impl Chain<'_> {
	/// Add a command.
	#[must_use]
	pub fn then(mut self, command: Command) -> Self {
		self.commands.push(command);
		self
	}

	/// Run every command in order, as one update.
	pub fn run(self, env: &Env<'_>) -> CommandResult {
		let Self { editor, commands } = self;
		let mut history = editor.detached_history(false);
		let update = editor
			.state
			.atomic(|state| run_all(&commands, state, &mut history, env))?;

		editor.history.close_group();
		let finished = editor.finish(update, env.now)?;
		editor.history.close_group();
		Ok(finished)
	}

	/// Whether every command would apply in order. Nothing changes.
	pub fn can(self, env: &Env<'_>) -> Result<bool, EditorError> {
		let Self { editor, commands } = self;
		let mut history = editor.detached_history(true);

		editor
			.state
			.dry_run(|state| run_all(&commands, state, &mut history, env))
	}
}

/// Run each command, composing the updates; stop at the first that does not
/// apply.
fn run_all(
	commands: &[Command],
	state: &mut EditorState,
	history: &mut dyn History,
	env: &Env<'_>,
) -> CommandResult {
	let mut combined: Option<Update> = None;

	for command in commands {
		let mut context = Context {
			state,
			history,
			lines: env.lines,
			last_rule: None,
		};

		let Some(update) = command(&mut context)? else {
			return Ok(None);
		};

		combined = Some(match combined {
			Some(previous) => previous.then(update),
			None => update,
		});
	}

	Ok(combined)
}
