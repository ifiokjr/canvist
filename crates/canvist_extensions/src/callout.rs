//! Callouts: a tinted box for notes, tips, warnings, and errors.

use canvist_editor::Command;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::InputRule;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::resolve;

const CALLOUT: &str = "callout";

/// Remirror's callout types.
pub(crate) const TYPES: [&str; 5] = ["info", "warning", "error", "success", "blank"];

/// Remirror's `callout` block: a box of content with a `type` (`info`,
/// `warning`, `error`, `success`, or `blank`) that the theme colours.
///
/// `toggleCallout` wraps the selection or lifts it out, `setCalloutType`
/// changes the type, and `:::warning ` at the start of a line makes one.
/// Markdown writes GitHub's alerts (`> [!WARNING]`) and reads them back.
#[derive(Clone, Copy, Debug, Default)]
pub struct Callout;

/// A callout type for [`CalloutHandle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CalloutType {
	/// A note: the default.
	Info,
	/// Something to be careful about.
	Warning,
	/// Something that went or will go wrong.
	Error,
	/// Something that worked, or a tip.
	Success,
	/// A plain box with no colour.
	Blank,
}

impl CalloutType {
	/// The attribute value, such as `warning`.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Info => "info",
			Self::Warning => "warning",
			Self::Error => "error",
			Self::Success => "success",
			Self::Blank => "blank",
		}
	}

	fn from_name(name: &str) -> Option<Self> {
		[
			Self::Info,
			Self::Warning,
			Self::Error,
			Self::Success,
			Self::Blank,
		]
		.into_iter()
		.find(|kind| kind.name() == name)
	}
}

/// Typed access to [`Callout`].
#[derive(Clone, Copy, Debug)]
pub struct CalloutHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl CalloutHandle {
	/// Wrap the selection in a callout of `kind`, or lift it out of the
	/// callout around it.
	#[must_use]
	pub fn toggle(self, kind: CalloutType) -> Command {
		toggle(kind.name())
	}

	/// Change the type of the callout around the selection.
	#[must_use]
	pub fn set_type(self, kind: CalloutType) -> Command {
		set_type(kind.name())
	}

	/// The type of the callout around the selection, if there is one.
	#[must_use]
	pub fn kind(self, state: &EditorState) -> Option<CalloutType> {
		type_around(state).and_then(CalloutType::from_name)
	}
}

/// The type of the callout around the selection, if there is one. A
/// missing type, or one this extension does not know, which JSON or a
/// collaborator may carry, reads as `info`, as the theme draws it.
fn type_around(state: &EditorState) -> Option<&'static str> {
	let callout = resolve::around(state, CALLOUT)?;
	let attrs = state.doc().block(callout)?.attrs();
	let named = attrs.get("type").and_then(Value::as_str);

	Some(
		TYPES
			.into_iter()
			.find(|kind| Some(*kind) == named)
			.unwrap_or("info"),
	)
}

fn toggle(kind: &'static str) -> Command {
	command(move |state| {
		let callout = resolve::block(state, CALLOUT, CALLOUT)?;
		commands::toggle_wrap(state, callout, Attrs::new().with("type", kind))
	})
}

fn set_type(kind: &'static str) -> Command {
	command(move |state| {
		let Some(block) = resolve::around(state, CALLOUT) else {
			return Ok(None);
		};
		let attrs = state
			.doc()
			.block(block)
			.map(|callout| callout.attrs().clone().with("type", kind))
			.unwrap_or_default();
		commands::set_block_attrs(state, block, &attrs)
	})
}

fn type_arg(args: &canvist_editor::Args<'_>) -> Result<&'static str, EditorError> {
	let name = args.string("type")?;
	TYPES.into_iter().find(|kind| *kind == name).ok_or_else(|| {
		EditorError::InvalidArgs {
			name: CALLOUT.to_owned(),
			reason: format!("no callout type `{name}`"),
		}
	})
}

fn type_field(required: bool) -> Field {
	let doc = "The callout type: info, warning, error, success, or blank.";
	let ty = ValueType::OneOf(TYPES.to_vec());

	if required {
		Field::required("type", ty, doc)
	} else {
		Field::optional("type", ty, "info", doc)
	}
}

impl Extension for Callout {
	fn name(&self) -> &'static str {
		CALLOUT
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(
			BlockSpec::container(CALLOUT, &["block"]).attrs(Attrs::new().with("type", "info")),
		)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, CALLOUT).attr(type_field(false))]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let toggle_spec = CommandSpec::new(
			"toggleCallout",
			"Wrap the selection in a callout, or lift it out of the callout around it.",
			|args| Ok(toggle(type_arg(args)?)),
		);
		let set_spec = CommandSpec::new(
			"setCalloutType",
			"Change the type of the callout around the selection.",
			|args| Ok(set_type(type_arg(args)?)),
		);

		Ok(vec![
			toggle_spec.arg(type_field(false)),
			set_spec.arg(type_field(true)),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"calloutType",
			"The type of the callout around the selection, or null outside one.",
			ValueType::nullable(ValueType::OneOf(TYPES.to_vec())),
			|state, _| Ok(type_around(state).map_or(Value::Null, Value::from)),
		)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let callout = canvist_editor::block_type(schema, CALLOUT, CALLOUT)?;
		let rule = InputRule::wrap(r"^:::([a-zA-Z]*)\s$", move |state, found| {
			let named = found.group(1).unwrap_or_default().to_ascii_lowercase();
			let kind = TYPES
				.into_iter()
				.find(|kind| *kind == named)
				.unwrap_or("info");
			commands::wrap_in(state, callout, Attrs::new().with("type", kind))
		})?;
		Ok(vec![rule])
	}
}

impl TypedExtension for Callout {
	type Handle = CalloutHandle;

	fn handle(&self, registration: Registration) -> CalloutHandle {
		CalloutHandle {
			_registration: registration,
		}
	}
}
