//! Typed canvas extensions: node types, tools, commands, and inspector
//! sections, registered once and reached through typed handles.

use std::ops::Range;
use std::sync::Arc;

use canvist_scene::SceneSchemaBuilder;
use serde_json::Value as Json;
use serde_json::json;

use crate::CanvasError;
use crate::CanvasState;
use crate::inspector::Field;
use crate::inspector::Section;
use crate::tools::Outcome;
use crate::tools::Tool;

/// Identifies a registered tool. Only registration hands them out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToolId(pub(crate) usize);

impl ToolId {
	/// The hand tool, always registered.
	pub const HAND: Self = Self(1);
	/// The select tool, always registered.
	pub const SELECT: Self = Self(0);

	/// The tool's position in [`crate::Canvas::tools`].
	#[must_use]
	pub fn index(self) -> usize {
		self.0
	}
}

/// Proof that an extension was registered, with the ids of the tools it
/// contributed, in the order its [`CanvasExtension::tools`] returned them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
	tools: Range<usize>,
}

impl Registration {
	/// The id of the extension's `nth` tool.
	///
	/// # Panics
	///
	/// Panics when the extension contributed fewer than `nth + 1` tools,
	/// which is a bug in the extension's handle.
	#[must_use]
	pub fn tool(&self, nth: usize) -> ToolId {
		let id = self.tools.start + nth;
		assert!(
			self.tools.contains(&id),
			"the extension registered no tool {nth}"
		);
		ToolId(id)
	}
}

/// Runs a named command with JSON arguments.
pub type CommandFn =
	Arc<dyn Fn(&mut CanvasState, &Json) -> Result<Outcome, CanvasError> + Send + Sync>;

/// A named command, callable from any binding by name and described by
/// metadata so bindings can type it.
#[derive(Clone)]
pub struct Command {
	/// The command name, such as `group`.
	pub name: String,
	/// What it does, for documentation and agents.
	pub description: String,
	/// Its arguments, as an object with these fields.
	pub args: Vec<Field>,
	/// The implementation.
	pub run: CommandFn,
}

impl std::fmt::Debug for Command {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Command")
			.field("name", &self.name)
			.field("args", &self.args)
			.finish_non_exhaustive()
	}
}

impl Command {
	/// A command named `name`.
	pub fn new(
		name: &str,
		description: &str,
		args: Vec<Field>,
		run: impl Fn(&mut CanvasState, &Json) -> Result<Outcome, CanvasError> + Send + Sync + 'static,
	) -> Self {
		Self {
			name: name.to_owned(),
			description: description.to_owned(),
			args,
			run: Arc::new(run),
		}
	}

	/// The command's metadata.
	#[must_use]
	pub fn meta(&self) -> Json {
		json!({
			"name": self.name,
			"description": self.description,
			"args": self.args.iter().map(Field::meta).collect::<Vec<_>>(),
		})
	}
}

/// An extension: anything a canvas app opts into.
pub trait CanvasExtension: Send + Sync {
	/// A unique name, such as `shapes`.
	fn name(&self) -> &'static str;

	/// Register node types.
	fn schema(&self, schema: SceneSchemaBuilder) -> SceneSchemaBuilder {
		schema
	}

	/// Fresh instances of the extension's tools, for one canvas.
	fn tools(&self) -> Vec<Box<dyn Tool>> {
		Vec::new()
	}

	/// The extension's commands.
	fn commands(&self) -> Vec<Command> {
		Vec::new()
	}

	/// The extension's inspector sections.
	fn inspector(&self) -> Vec<Section> {
		Vec::new()
	}
}

/// An extension with a typed handle, returned by
/// [`CanvasBuilder::add`](crate::CanvasBuilder::add). Calling a tool or
/// command of an extension the app never registered does not compile.
pub trait TypedCanvasExtension: CanvasExtension + Sized + 'static {
	/// The handle: tool ids and command methods.
	type Handle: Copy;

	/// The handle, given proof of registration.
	fn handle(&self, registration: Registration) -> Self::Handle;
}

impl Registration {
	pub(crate) fn new(tools: Range<usize>) -> Self {
		Self { tools }
	}
}
