//! Links.

use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::PasteRule;
use canvist_editor::RuleMatch;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::mark_type;
use canvist_model::Attrs;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::MarkTypeId;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

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

/// Link the selection to `href`, or update the link it already has.
pub fn set_link(state: &mut EditorState, href: &str) -> CommandResult {
	let kind = kind(state.schema())?;
	commands::set_mark(state, &link(kind, href))
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

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		let kind = kind(schema)?;
		Ok(vec![(
			"unsetLink",
			command(move |state| commands::unset_mark(state, kind)),
		)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		if !self.autolink {
			return Ok(Vec::new());
		}

		let kind = kind(schema)?;
		let pattern = format!(r"(?:^|\s)({URL})[.,;:!?)\]]*\s$");
		Ok(vec![InputRule::new(&pattern, move |state, found| {
			autolink(state, found, kind)
		})?])
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
