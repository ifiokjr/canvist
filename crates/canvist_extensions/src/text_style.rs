//! Inline styles: the `textStyle` mark and the extensions that give it
//! attributes, as in TipTap's `TextStyleKit`.
//!
//! [`TextStyle`] registers a `textStyle` mark with no attributes of its own.
//! [`Color`], [`BackgroundColor`], [`FontFamily`], [`FontSize`], and
//! [`LineHeight`] each add one attribute to it, with a command to set it and
//! one to clear it. A run carries at most one `textStyle` mark, so setting
//! its colour keeps its font size, and clearing its last attribute removes
//! the mark.

#![cfg_attr(
	not(any(
		feature = "background-color",
		feature = "color",
		feature = "font-family",
		feature = "font-size",
		feature = "line-height"
	)),
	allow(
		dead_code,
		unused_macros,
		reason = "only the attribute extensions use the attribute machinery"
	)
)]

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
use canvist_model::MarkSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::resolve;

/// The mark every style attribute lives on.
const TEXT_STYLE: &str = "textStyle";

/// The `textStyle` mark: a `<span style>` that [`Color`], [`FontFamily`],
/// and the other style extensions add attributes to. Register it with at
/// least one of them.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextStyle;

/// Typed access to [`TextStyle`].
#[derive(Clone, Copy, Debug)]
pub struct TextStyleHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl TextStyleHandle {
	/// Remove `textStyle` marks whose attributes are all unset.
	#[must_use]
	pub fn remove_empty(self) -> Command {
		remove_empty()
	}
}

/// Whether every attribute is unset: absent or null.
fn is_empty(attrs: &Attrs) -> bool {
	attrs.iter().all(|(_, value)| *value == Value::Null)
}

fn remove_empty() -> Command {
	command(|state| {
		let kind = resolve::mark(state, TEXT_STYLE, TEXT_STYLE)?;
		let update = |current: Option<&Attrs>| current.filter(|attrs| !is_empty(attrs)).cloned();
		commands::update_mark_attrs(state, kind, &update)
	})
}

impl Extension for TextStyle {
	fn name(&self) -> &'static str {
		TEXT_STYLE
	}

	/// Above the default, so the style span wraps other marks, as in TipTap.
	fn priority(&self) -> i32 {
		101
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new(TEXT_STYLE))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, TEXT_STYLE)]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"removeEmptyTextStyle",
			"Remove text styles whose attributes are all unset.",
			remove_empty(),
		)])
	}
}

impl TypedExtension for TextStyle {
	type Handle = TextStyleHandle;

	fn handle(&self, registration: Registration) -> TextStyleHandle {
		TextStyleHandle {
			_registration: registration,
		}
	}
}

/// Everything that distinguishes one style attribute from another.
#[derive(Debug)]
struct StyleAttr {
	/// The extension name.
	extension: &'static str,
	/// The attribute on `textStyle`, which is also the query name.
	attr: &'static str,
	/// The set and unset command names.
	commands: [&'static str; 2],
	/// What the attribute holds, for its argument docs.
	doc: &'static str,
}

/// Typed access to a style attribute extension such as [`Color`].
#[derive(Clone, Copy, Debug)]
pub struct StyleAttrHandle {
	def: &'static StyleAttr,
	_registration: Registration,
}

impl StyleAttrHandle {
	/// Set the attribute on the selection, keeping its other styles. At a
	/// caret, it applies to the next typed text.
	#[must_use]
	pub fn set(self, value: &str) -> Command {
		self.def.set(value)
	}

	/// Clear the attribute from the selection, removing text styles left
	/// with no attributes.
	#[must_use]
	pub fn unset(self) -> Command {
		self.def.unset()
	}

	/// The value every selected character shares, if they share one.
	#[must_use]
	pub fn value(self, state: &EditorState) -> Option<String> {
		self.def.value(state)
	}
}

impl StyleAttr {
	fn set(&'static self, value: &str) -> Command {
		let value = Value::from(value);
		command(move |state| {
			let kind = resolve::mark(state, self.extension, TEXT_STYLE)?;
			let update = |current: Option<&Attrs>| {
				let attrs = current.cloned().unwrap_or_default();
				Some(attrs.with(self.attr, value.clone()))
			};
			commands::update_mark_attrs(state, kind, &update)
		})
	}

	/// Remove the attribute (an absent attribute is its default, null), and
	/// the mark with it when nothing else is set.
	fn unset(&'static self) -> Command {
		command(move |state| {
			let kind = resolve::mark(state, self.extension, TEXT_STYLE)?;
			let update = |current: Option<&Attrs>| {
				let mut attrs = current?.clone();
				attrs.remove(self.attr);
				(!is_empty(&attrs)).then_some(attrs)
			};
			commands::update_mark_attrs(state, kind, &update)
		})
	}

	fn value(&self, state: &EditorState) -> Option<String> {
		let value = resolve::shared_mark_attr(state, TEXT_STYLE, self.attr)?;
		value.as_str().map(str::to_owned)
	}

	fn field(&self, required: bool) -> Field {
		if required {
			Field::required(self.attr, ValueType::String, self.doc)
		} else {
			Field::optional(
				self.attr,
				ValueType::nullable(ValueType::String),
				Value::Null,
				self.doc,
			)
		}
	}

	fn node(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, TEXT_STYLE).attr(self.field(false))]
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark_attrs(&[TEXT_STYLE], Attrs::new().with(self.attr, Value::Null))
	}

	fn commands(&'static self) -> Vec<CommandSpec> {
		let [set, unset] = self.commands;
		let set_spec = CommandSpec::new(
			set,
			"Set the style on the selection, keeping its other styles.",
			move |args| Ok(self.set(args.string(self.attr)?)),
		);

		vec![
			set_spec.arg(self.field(true)),
			CommandSpec::simple(unset, "Clear the style from the selection.", self.unset()),
		]
	}

	fn queries(&'static self) -> Vec<QuerySpec> {
		vec![QuerySpec::new(
			self.attr,
			"The value every selected character shares, or null.",
			ValueType::nullable(ValueType::String),
			move |state, _| Ok(self.value(state).map_or(Value::Null, Value::from)),
		)]
	}

	fn handle(&'static self, registration: Registration) -> StyleAttrHandle {
		StyleAttrHandle {
			def: self,
			_registration: registration,
		}
	}
}

#[cfg(feature = "color")]
static COLOR: StyleAttr = StyleAttr {
	extension: "color",
	attr: "color",
	commands: ["setColor", "unsetColor"],
	doc: "The text colour, any CSS colour such as `#958df1`.",
};

#[cfg(feature = "background-color")]
static BACKGROUND_COLOR: StyleAttr = StyleAttr {
	extension: "backgroundColor",
	attr: "backgroundColor",
	commands: ["setBackgroundColor", "unsetBackgroundColor"],
	doc: "The background colour behind the text, any CSS colour.",
};

#[cfg(feature = "font-family")]
static FONT_FAMILY: StyleAttr = StyleAttr {
	extension: "fontFamily",
	attr: "fontFamily",
	commands: ["setFontFamily", "unsetFontFamily"],
	doc: "The CSS font family list, such as `Inter, sans-serif`.",
};

#[cfg(feature = "font-size")]
static FONT_SIZE: StyleAttr = StyleAttr {
	extension: "fontSize",
	attr: "fontSize",
	commands: ["setFontSize", "unsetFontSize"],
	doc: "The font size, such as `18px`.",
};

#[cfg(feature = "line-height")]
static LINE_HEIGHT: StyleAttr = StyleAttr {
	extension: "lineHeight",
	attr: "lineHeight",
	commands: ["setLineHeight", "unsetLineHeight"],
	doc: "The line height as a multiple of the font size, such as `1.5`.",
};

/// Declares a style attribute extension: a unit struct whose [`Extension`]
/// and [`TypedExtension`] delegate to its [`StyleAttr`].
macro_rules! style_attr {
	($(#[$meta:meta])* $type:ident, $def:expr) => {
		$(#[$meta])*
		#[derive(Clone, Copy, Debug, Default)]
		pub struct $type;

		impl Extension for $type {
			fn name(&self) -> &'static str {
				$def.extension
			}

			fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
				$def.schema(schema)
			}

			fn nodes(&self) -> Vec<NodeMeta> {
				$def.node()
			}

			fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
				Ok($def.commands())
			}

			fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
				Ok($def.queries())
			}
		}

		impl TypedExtension for $type {
			type Handle = StyleAttrHandle;

			fn handle(&self, registration: Registration) -> StyleAttrHandle {
				$def.handle(registration)
			}
		}
	};
}

#[cfg(feature = "color")]
style_attr!(
	/// The text colour, as a `color` attribute on `textStyle`: `setColor`,
	/// `unsetColor`.
	Color,
	COLOR
);

#[cfg(feature = "background-color")]
style_attr!(
	/// The background behind text, as a `backgroundColor` attribute on
	/// `textStyle`: `setBackgroundColor`, `unsetBackgroundColor`.
	BackgroundColor,
	BACKGROUND_COLOR
);

#[cfg(feature = "font-family")]
style_attr!(
	/// The font family, as a `fontFamily` attribute on `textStyle`:
	/// `setFontFamily`, `unsetFontFamily`.
	FontFamily,
	FONT_FAMILY
);

#[cfg(feature = "font-size")]
style_attr!(
	/// The font size, as a `fontSize` attribute on `textStyle`:
	/// `setFontSize`, `unsetFontSize`.
	FontSize,
	FONT_SIZE
);

#[cfg(feature = "line-height")]
style_attr!(
	/// The line height, as a `lineHeight` attribute on `textStyle`:
	/// `setLineHeight`, `unsetLineHeight`.
	LineHeight,
	LINE_HEIGHT
);
