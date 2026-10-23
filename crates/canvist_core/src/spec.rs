//! Command and query specifications: one declaration that both runs and
//! describes itself, so the metadata bindings read can never disagree with
//! what the editor does.

use std::sync::Arc;

use canvist_model::Attrs;
use canvist_model::Value;

use crate::Command;
use crate::CommandMeta;
use crate::EditorError;
use crate::EditorState;
use crate::Field;
use crate::QueryMeta;
use crate::ValueType;

/// Arguments checked against a command's or query's declared fields.
#[derive(Clone, Copy, Debug)]
pub struct Args<'a> {
	owner: &'static str,
	fields: &'a [Field],
	given: &'a Attrs,
}

impl<'a> Args<'a> {
	/// Check `given` against `fields`: every required field present, every
	/// value of the declared type, and nothing undeclared.
	fn check(
		owner: &'static str,
		fields: &'a [Field],
		given: &'a Attrs,
	) -> Result<Self, EditorError> {
		let invalid = |reason: String| {
			EditorError::InvalidArgs {
				name: owner.to_owned(),
				reason,
			}
		};

		if let Some((unknown, _)) = given
			.iter()
			.find(|(key, _)| !fields.iter().any(|field| field.name == *key))
		{
			return Err(invalid(format!("unknown argument `{unknown}`")));
		}

		for field in fields {
			match (given.get(field.name), &field.default) {
				(Some(value), _) if !field.ty.accepts(value) => {
					return Err(invalid(format!("`{}` must be {}", field.name, field.ty)));
				}
				(None, None) => return Err(invalid(format!("`{}` is required", field.name))),
				_ => {}
			}
		}

		Ok(Self {
			owner,
			fields,
			given,
		})
	}

	/// The value of `name`, or its default.
	pub fn get(&self, name: &str) -> Result<&'a Value, EditorError> {
		let field = self.fields.iter().find(|field| field.name == name);
		let value = field.and_then(|field| self.given.get(name).or(field.default.as_ref()));

		value.ok_or_else(|| {
			EditorError::InvalidArgs {
				name: self.owner.to_owned(),
				reason: format!("`{name}` is not an argument"),
			}
		})
	}

	/// An integer argument.
	pub fn int(&self, name: &str) -> Result<i64, EditorError> {
		self.get(name)?.as_int().ok_or_else(|| self.mistyped(name))
	}

	/// A boolean argument.
	pub fn bool(&self, name: &str) -> Result<bool, EditorError> {
		self.get(name)?.as_bool().ok_or_else(|| self.mistyped(name))
	}

	/// A string argument.
	pub fn string(&self, name: &str) -> Result<&'a str, EditorError> {
		self.get(name)?.as_str().ok_or_else(|| self.mistyped(name))
	}

	/// A string argument that may be null.
	pub fn optional_string(&self, name: &str) -> Result<Option<&'a str>, EditorError> {
		Ok(self.get(name)?.as_str())
	}

	fn mistyped(&self, name: &str) -> EditorError {
		EditorError::InvalidArgs {
			name: self.owner.to_owned(),
			reason: format!("`{name}` has the wrong type"),
		}
	}
}

/// Builds a command from checked arguments.
pub type CommandBuilder = Arc<dyn Fn(&Args<'_>) -> Result<Command, EditorError> + Send + Sync>;

/// A named command an extension offers, with typed arguments.
#[derive(Clone)]
pub struct CommandSpec {
	meta: CommandMeta,
	build: CommandBuilder,
}

impl std::fmt::Debug for CommandSpec {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("CommandSpec")
			.field("meta", &self.meta)
			.finish_non_exhaustive()
	}
}

impl CommandSpec {
	/// A command whose behaviour depends on its arguments.
	pub fn new(
		name: &'static str,
		doc: &'static str,
		build: impl Fn(&Args<'_>) -> Result<Command, EditorError> + Send + Sync + 'static,
	) -> Self {
		Self {
			meta: CommandMeta {
				name,
				doc: crate::meta::doc(doc),
				args: Vec::new(),
			},
			build: Arc::new(build),
		}
	}

	/// A command without arguments.
	#[must_use]
	pub fn simple(name: &'static str, doc: &'static str, command: Command) -> Self {
		Self::new(name, doc, move |_| Ok(command.clone()))
	}

	/// Declare an argument.
	#[must_use]
	pub fn arg(mut self, field: Field) -> Self {
		self.meta.args.push(field);
		self
	}

	/// The command's description.
	#[must_use]
	pub fn meta(&self) -> &CommandMeta {
		&self.meta
	}

	/// The command for `args`, after checking them against the declared
	/// arguments.
	pub fn build(&self, args: &Attrs) -> Result<Command, EditorError> {
		let args = Args::check(self.meta.name, &self.meta.args, args)?;
		(self.build)(&args)
	}
}

/// Answers a query from the state and checked arguments.
pub type QueryFn = Arc<dyn Fn(&EditorState, &Args<'_>) -> Result<Value, EditorError> + Send + Sync>;

/// A named question an extension can answer about the state, such as
/// whether bold is active or the heading level at the caret.
#[derive(Clone)]
pub struct QuerySpec {
	meta: QueryMeta,
	run: QueryFn,
}

impl std::fmt::Debug for QuerySpec {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("QuerySpec")
			.field("meta", &self.meta)
			.finish_non_exhaustive()
	}
}

impl QuerySpec {
	/// A query returning a value of type `returns`.
	pub fn new(
		name: &'static str,
		doc: &'static str,
		returns: ValueType,
		run: impl Fn(&EditorState, &Args<'_>) -> Result<Value, EditorError> + Send + Sync + 'static,
	) -> Self {
		Self {
			meta: QueryMeta {
				name,
				doc: crate::meta::doc(doc),
				args: Vec::new(),
				returns,
			},
			run: Arc::new(run),
		}
	}

	/// Declare an argument.
	#[must_use]
	pub fn arg(mut self, field: Field) -> Self {
		self.meta.args.push(field);
		self
	}

	/// The query's description.
	#[must_use]
	pub fn meta(&self) -> &QueryMeta {
		&self.meta
	}

	/// Answer the query for `args`, after checking them.
	pub fn run(&self, state: &EditorState, args: &Attrs) -> Result<Value, EditorError> {
		let args = Args::check(self.meta.name, &self.meta.args, args)?;
		(self.run)(state, &args)
	}
}
