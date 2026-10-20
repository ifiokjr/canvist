//! Kinetic type: text that moves, glyph by glyph, on the canvas.

use canvist_editor::Command;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
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

const KINETIC: &str = "kinetic";
const EFFECT: &str = "effect";

/// How kinetic text moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KineticEffect {
	/// Each glyph rises and falls a little after the one before it.
	#[default]
	Wave,
	/// Glyphs jitter, as if shivering.
	Shake,
	/// Glyphs fade in and out in a ripple.
	Pulse,
	/// Glyphs cycle through the colour wheel.
	Rainbow,
}

impl KineticEffect {
	/// Every effect.
	pub const ALL: [Self; 4] = [Self::Wave, Self::Shake, Self::Pulse, Self::Rainbow];

	/// The attribute value, such as `wave`, which `canvist_layout`'s
	/// `Motion` reads.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Wave => "wave",
			Self::Shake => "shake",
			Self::Pulse => "pulse",
			Self::Rainbow => "rainbow",
		}
	}

	fn from_name(name: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|effect| effect.name() == name)
	}
}

/// A canvas-native extension: the `kinetic` mark, whose `effect` makes its
/// text wave, shake, pulse, or cycle through colours.
///
/// The document stores only the mark; the motion happens when a host draws
/// a display list with a time. `canvist_layout` then places each glyph of
/// kinetic text where its effect puts it at that moment, and reports that
/// the list is animated, so the host keeps asking for frames while kinetic
/// text is on screen and stops when it scrolls away. Hosts pass no time to
/// readers who prefer reduced motion, and the text stays still. HTML writes
/// `<span data-kinetic="wave">`, which `canvist_layout`'s server renderer
/// animates with CSS; Markdown keeps only the text.
#[derive(Clone, Copy, Debug, Default)]
pub struct Kinetic;

/// Typed access to [`Kinetic`].
#[derive(Clone, Copy, Debug)]
pub struct KineticHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl KineticHandle {
	/// Make the selection move with `effect`.
	#[must_use]
	pub fn set(self, effect: KineticEffect) -> Command {
		set(effect)
	}

	/// Make the selection still.
	#[must_use]
	pub fn unset(self) -> Command {
		command(|state| {
			let kind = resolve::mark(state, KINETIC, KINETIC)?;
			commands::unset_mark(state, kind)
		})
	}

	/// Make the selection move with `effect`, or still when it already does.
	#[must_use]
	pub fn toggle(self, effect: KineticEffect) -> Command {
		toggle(effect)
	}

	/// The effect of the text at the selection, if it moves.
	#[must_use]
	pub fn effect(self, state: &EditorState) -> Option<KineticEffect> {
		resolve::shared_mark_attr(state, KINETIC, EFFECT)
			.and_then(|value| value.as_str().and_then(KineticEffect::from_name))
	}
}

fn mark(state: &EditorState, effect: KineticEffect) -> Result<Mark, EditorError> {
	let kind = resolve::mark(state, KINETIC, KINETIC)?;
	Ok(Mark::with_attrs(
		kind,
		Attrs::new().with(EFFECT, effect.name()),
	))
}

fn set(effect: KineticEffect) -> Command {
	command(move |state| {
		let mark = mark(state, effect)?;
		commands::set_mark(state, &mark)
	})
}

fn toggle(effect: KineticEffect) -> Command {
	command(move |state| {
		let mark = mark(state, effect)?;
		commands::toggle_mark(state, &mark)
	})
}

fn effect_arg(args: &canvist_editor::Args<'_>) -> Result<KineticEffect, EditorError> {
	let name = args.string(EFFECT)?;

	KineticEffect::from_name(name).ok_or_else(|| {
		EditorError::InvalidArgs {
			name: KINETIC.to_owned(),
			reason: format!("no kinetic effect `{name}`"),
		}
	})
}

fn effect_type() -> ValueType {
	ValueType::OneOf(
		KineticEffect::ALL
			.iter()
			.map(|effect| effect.name())
			.collect(),
	)
}

fn effect_field() -> Field {
	Field::optional(
		EFFECT,
		effect_type(),
		KineticEffect::Wave.name(),
		"How the text moves: wave, shake, pulse, or rainbow.",
	)
}

impl Extension for Kinetic {
	fn name(&self) -> &'static str {
		KINETIC
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(
			MarkSpec::new(KINETIC).attrs(Attrs::new().with(EFFECT, KineticEffect::Wave.name())),
		)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, KINETIC).attr(effect_field())]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![
			CommandSpec::new("setKinetic", "Make the selection move.", |args| {
				Ok(set(effect_arg(args)?))
			})
			.arg(effect_field()),
			CommandSpec::new(
				"toggleKinetic",
				"Make the selection move, or still when it already does.",
				|args| Ok(toggle(effect_arg(args)?)),
			)
			.arg(effect_field()),
			CommandSpec::simple(
				"unsetKinetic",
				"Make the selection still.",
				command(|state| {
					let kind = resolve::mark(state, KINETIC, KINETIC)?;
					commands::unset_mark(state, kind)
				}),
			),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"kineticEffect",
			"How the text at the selection moves, or null when it is still.",
			ValueType::nullable(effect_type()),
			|state, _| {
				Ok(resolve::shared_mark_attr(state, KINETIC, EFFECT)
					.filter(|value| {
						value
							.as_str()
							.is_some_and(|name| KineticEffect::from_name(name).is_some())
					})
					.unwrap_or(Value::Null))
			},
		)])
	}
}

impl TypedExtension for Kinetic {
	type Handle = KineticHandle;

	fn handle(&self, registration: Registration) -> KineticHandle {
		KineticHandle {
			_registration: registration,
		}
	}
}
