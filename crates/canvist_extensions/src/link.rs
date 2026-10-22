//! Links.

use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::InputRule;
use canvist_core::Lookback;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::PasteRule;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::RuleMatch;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_model::Attrs;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::MarkTypeId;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

use crate::resolve;

/// A URL that does not swallow trailing punctuation.
const URL: &str = r#"https?://[^\s<>"']*[^\s<>"'.,;:!?)\]]"#;

/// A non-inclusive `link` mark with an `href`, so typing at a link's edge
/// does not extend it. URLs become links when pasted and when followed by
/// a space.
#[derive(Clone, Copy, Debug)]
pub struct Link {
	/// Link URLs typed into the text.
	pub autolink: bool,
	/// Link URLs in pasted text.
	pub link_on_paste: bool,
}

impl Default for Link {
	fn default() -> Self {
		Self {
			autolink: true,
			link_on_paste: true,
		}
	}
}

fn kind(schema: &Schema) -> Result<MarkTypeId, EditorError> {
	mark_type(schema, "link", "link")
}

fn link(kind: MarkTypeId, href: &str) -> Mark {
	Mark::with_attrs(kind, Attrs::new().with("href", href))
}

/// Typed access to [`Link`].
#[derive(Clone, Copy, Debug)]
pub struct LinkHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl LinkHandle {
	/// Link the selection to `href`, or update the link it already has. At a
	/// caret, the next typed text is linked.
	#[must_use]
	pub fn set(self, href: &str) -> Command {
		set(href)
	}

	/// Remove links from the selection.
	#[must_use]
	pub fn unset(self) -> Command {
		unset()
	}

	/// The `href` of the link at the selection, if every selected character
	/// shares one.
	#[must_use]
	pub fn href(self, state: &EditorState) -> Option<String> {
		href(state)
	}
}

fn set(href: &str) -> Command {
	let href = href.to_owned();
	command(move |state| {
		let kind = resolve::mark(state, "link", "link")?;
		commands::set_mark(state, &link(kind, &href))
	})
}

fn unset() -> Command {
	command(|state| {
		let kind = resolve::mark(state, "link", "link")?;
		commands::unset_mark(state, kind)
	})
}

/// The link at the selection: the active marks at a caret, or the marks
/// every selected character shares.
fn href(state: &EditorState) -> Option<String> {
	let kind = state.schema().mark_type("link")?;

	if !commands::is_mark_active(state, kind) {
		return None;
	}

	let marks = commands::active_marks(state);
	let href = marks.get(kind)?.attrs.get("href")?.as_str()?;
	Some(href.to_owned())
}

fn href_field(required: bool) -> Field {
	let doc = "The link target.";

	if required {
		Field::required("href", ValueType::String, doc)
	} else {
		Field::optional(
			"href",
			ValueType::nullable(ValueType::String),
			Value::Null,
			doc,
		)
	}
}

/// Link the URL before the caret (leaving trailing punctuation outside the
/// link), then type the space that ended it.
fn autolink(state: &mut EditorState, found: &RuleMatch, kind: MarkTypeId) -> CommandResult {
	let (Some(Some(url)), Some(href)) = (found.groups.get(1).cloned(), found.group(1)) else {
		return Ok(None);
	};
	let mark = link(kind, href);
	let caret = Point::new(found.block, found.caret);
	let marks = commands::marks_at(state.doc(), caret).without(kind);

	state.change(EditKind::InputRule, |tr| {
		tr.add_mark(found.block, url, mark)?;
		let end = tr.insert_text(caret, &found.typed, marks)?;

		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

impl Extension for Link {
	fn name(&self) -> &'static str {
		"link"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = Attrs::new().with("href", Value::Null);
		schema.mark(MarkSpec::new("link").inclusive(false).attrs(attrs))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, "link").attr(href_field(false))]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let set_spec = CommandSpec::new(
			"setLink",
			"Link the selection to a URL, or update the link it already has.",
			|args| Ok(set(args.string("href")?)),
		);
		Ok(vec![
			set_spec.arg(href_field(true)),
			CommandSpec::simple("unsetLink", "Remove links from the selection.", unset()),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let query = QuerySpec::new(
			"linkHref",
			"The href of the link at the selection, or null.",
			ValueType::nullable(ValueType::String),
			|state, _| Ok(href(state).map_or(Value::Null, Value::from)),
		);
		Ok(vec![query])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		if !self.autolink {
			return Ok(Vec::new());
		}

		let kind = kind(schema)?;
		let pattern = format!(r"(?:^|\s)({URL})[.,;:!?)\]]*\s$");
		let rule = InputRule::new(&pattern, move |state, found| autolink(state, found, kind))?;
		Ok(vec![rule.lookback(Lookback::Word)])
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		if !self.link_on_paste {
			return Ok(Vec::new());
		}

		let kind = kind(schema)?;
		Ok(vec![PasteRule::mark_with(URL, kind, |url| {
			Attrs::new().with("href", url)
		})?])
	}
}

impl TypedExtension for Link {
	type Handle = LinkHandle;

	fn handle(&self, registration: Registration) -> LinkHandle {
		LinkHandle {
			_registration: registration,
		}
	}
}
