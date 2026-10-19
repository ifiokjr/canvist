//! StarterKit extensions registered by name, for bindings.
//!
//! Schema items are compiled in (architecture Decision 7), so a binding
//! carries every StarterKit extension and its host language chooses which
//! ones an engine registers. The names are the extension names in the
//! metadata.

use canvist_editor::BaseKeymap;
use canvist_editor::EditorBuilder;
use canvist_editor::EditorError;
use canvist_editor::UndoInputRule;
use canvist_extensions::Blockquote;
use canvist_extensions::Bold;
use canvist_extensions::BulletList;
use canvist_extensions::Code;
use canvist_extensions::CodeBlock;
use canvist_extensions::Document;
use canvist_extensions::HardBreak;
use canvist_extensions::Heading;
use canvist_extensions::History;
use canvist_extensions::HorizontalRule;
use canvist_extensions::Italic;
use canvist_extensions::Link;
use canvist_extensions::ListItem;
use canvist_extensions::OrderedList;
use canvist_extensions::Paragraph;
use canvist_extensions::Strike;
use canvist_extensions::TaskList;
use canvist_extensions::TrailingNode;
use canvist_extensions::Underline;

use crate::Engine;
use crate::HeadlessError;

/// Extensions every engine has: the root, paragraphs, the standard keys,
/// and Backspace undoing an input rule.
const ALWAYS: [&str; 4] = ["doc", "paragraph", "baseKeymap", "undoInputRule"];

/// Extensions an engine may register.
const OPTIONAL: [&str; 17] = [
	"heading",
	"blockquote",
	"codeBlock",
	"hardBreak",
	"horizontalRule",
	"bulletList",
	"orderedList",
	"listItem",
	"taskList",
	"bold",
	"italic",
	"underline",
	"strike",
	"code",
	"link",
	"history",
	"trailingNode",
];

/// Marks inline code excludes when they are registered, as the StarterKit
/// does.
const EXCLUDED_BY_CODE: [&str; 5] = ["bold", "italic", "underline", "strike", "link"];

/// A validated set of extension names.
#[derive(Clone, Debug)]
struct Extensions {
	names: Vec<&'static str>,
}

impl Extensions {
	/// Check `names` against the extensions this build has.
	fn parse(names: &[String]) -> Result<Self, EditorError> {
		let mut found = Vec::new();

		for name in names {
			if ALWAYS.contains(&name.as_str()) {
				continue;
			}

			let known = OPTIONAL
				.iter()
				.find(|known| **known == name)
				.ok_or_else(|| EditorError::UnknownName(name.clone()))?;

			if !found.contains(known) {
				found.push(*known);
			}
		}

		Ok(Self { names: found })
	}

	fn has(&self, name: &str) -> bool {
		self.names.contains(&name)
	}

	/// Register the extensions on `builder`.
	fn register(&self, builder: &mut EditorBuilder) {
		builder.add(Document);
		builder.add(Paragraph);
		builder.add(BaseKeymap);
		builder.add(UndoInputRule);

		for name in &self.names {
			self.register_one(builder, name);
		}

		let lists = self.has("bulletList") || self.has("orderedList");

		if lists && !self.has("listItem") {
			builder.add(ListItem);
		}
	}

	fn register_one(&self, builder: &mut EditorBuilder, name: &str) {
		match name {
			"heading" => {
				builder.add(Heading::default());
			}
			"blockquote" => {
				builder.add(Blockquote);
			}
			"codeBlock" => {
				builder.add(CodeBlock);
			}
			"hardBreak" => {
				builder.add(HardBreak);
			}
			"horizontalRule" => {
				builder.add(HorizontalRule);
			}
			"bulletList" => {
				builder.add(BulletList);
			}
			"orderedList" => {
				builder.add(OrderedList);
			}
			"listItem" => {
				builder.add(ListItem);
			}
			"taskList" => {
				builder.add(TaskList);
			}
			"bold" => {
				builder.add(Bold);
			}
			"italic" => {
				builder.add(Italic);
			}
			"underline" => {
				builder.add(Underline);
			}
			"strike" => {
				builder.add(Strike);
			}
			"link" => {
				builder.add(Link::default());
			}
			"history" => {
				builder.add(History);
			}
			"trailingNode" => {
				builder.add(TrailingNode);
			}
			"code" => {
				let excludes = EXCLUDED_BY_CODE
					.into_iter()
					.filter(|mark| self.has(mark))
					.collect();
				builder.add(Code { excludes });
			}
			_ => unreachable!("`Extensions::parse` admits only known names"),
		}
	}
}

impl Engine {
	/// An engine with the StarterKit extensions named in `names`, such as
	/// `["bold", "heading"]`, plus the document, paragraphs, and standard
	/// keys every engine has. Bindings use this to let their host language
	/// pick extensions; Rust code registers typed extensions with
	/// [`Engine::new`] instead.
	///
	/// ```
	/// use canvist_headless::Engine;
	///
	/// let engine = Engine::named(&["bold", "heading"])?;
	/// let doc = engine.parse(canvist_headless::Format::Markdown, "# Hi")?;
	/// assert_eq!(doc.export(canvist_headless::Format::Html), "<h1>Hi</h1>");
	/// # Ok::<(), canvist_headless::HeadlessError>(())
	/// ```
	pub fn named(names: &[impl AsRef<str>]) -> Result<Self, HeadlessError> {
		let names: Vec<String> = names.iter().map(|name| name.as_ref().to_owned()).collect();
		let extensions = Extensions::parse(&names).map_err(HeadlessError::Config)?;
		Self::new(move |builder| extensions.register(builder))
	}
}
