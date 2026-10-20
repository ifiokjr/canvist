//! Extensions registered by name, for bindings.
//!
//! Schema items are compiled in (architecture Decision 7), so a binding
//! carries every StarterKit extension, plus every other extension with the
//! `extras` feature, and its host language chooses which ones an engine
//! registers. The names are the extension names in the metadata.

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

/// StarterKit extensions an engine may register.
const STARTER_KIT: [&str; 17] = [
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

/// Extensions beyond the StarterKit, compiled with the `extras` feature.
#[cfg(feature = "extras")]
const EXTRA: [&str; 25] = [
	"subscript",
	"superscript",
	"highlight",
	"textStyle",
	"color",
	"backgroundColor",
	"fontFamily",
	"fontSize",
	"lineHeight",
	"textAlign",
	"typography",
	"characterCount",
	"placeholder",
	"mention",
	"hashtag",
	"emoji",
	"image",
	"youtube",
	"twitch",
	"audio",
	"file",
	"details",
	"callout",
	"mathematics",
	"table",
];

#[cfg(not(feature = "extras"))]
const EXTRA: [&str; 0] = [];

/// Marks inline code excludes when they are registered, as the StarterKit
/// does.
const EXCLUDED_BY_CODE: [&str; 8] = [
	"bold",
	"italic",
	"underline",
	"strike",
	"link",
	"subscript",
	"superscript",
	"highlight",
];

/// A validated set of extension names.
#[derive(Clone, Debug)]
struct Extensions {
	names: Vec<&'static str>,
}

impl Extensions {
	/// Check `names` against the extensions this build has.
	fn parse(names: &[String]) -> Result<Self, EditorError> {
		for name in names {
			let name = name.as_str();

			if !(ALWAYS.contains(&name) || STARTER_KIT.contains(&name) || EXTRA.contains(&name)) {
				return Err(EditorError::UnknownName(name.to_owned()));
			}
		}

		// Register in one fixed order, whatever order the names came in.
		// Marks nest and serialize in registration order, so without this
		// the same names build different schemas: Dart's alphabetical
		// `Extensions.all` would write `<mark><sub>` where TypeScript writes
		// `<sub><mark>`. Taking the known names in order, rather than
		// sorting the given ones, keeps a sort out of every binary.
		let mut found = Vec::new();

		for known in STARTER_KIT.iter().chain(EXTRA.iter()) {
			if names.iter().any(|name| name == known) {
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
			_ => register_extra(builder, name),
		}
	}
}

/// Register one of the extensions beyond the StarterKit.
#[cfg(feature = "extras")]
fn register_extra(builder: &mut EditorBuilder, name: &str) {
	use canvist_extensions::Audio;
	use canvist_extensions::BackgroundColor;
	use canvist_extensions::Callout;
	use canvist_extensions::CharacterCount;
	use canvist_extensions::Color;
	use canvist_extensions::Details;
	use canvist_extensions::Emoji;
	use canvist_extensions::File;
	use canvist_extensions::FontFamily;
	use canvist_extensions::FontSize;
	use canvist_extensions::Hashtag;
	use canvist_extensions::Highlight;
	use canvist_extensions::Image;
	use canvist_extensions::LineHeight;
	use canvist_extensions::Mathematics;
	use canvist_extensions::Mention;
	use canvist_extensions::Placeholder;
	use canvist_extensions::Subscript;
	use canvist_extensions::Superscript;
	use canvist_extensions::Table;
	use canvist_extensions::TextAlign;
	use canvist_extensions::TextStyle;
	use canvist_extensions::Twitch;
	use canvist_extensions::Typography;
	use canvist_extensions::Youtube;

	match name {
		"subscript" => {
			builder.add(Subscript);
		}
		"superscript" => {
			builder.add(Superscript);
		}
		"highlight" => {
			builder.add(Highlight::default());
		}
		"textStyle" => {
			builder.add(TextStyle);
		}
		"color" => {
			builder.add(Color);
		}
		"backgroundColor" => {
			builder.add(BackgroundColor);
		}
		"fontFamily" => {
			builder.add(FontFamily);
		}
		"fontSize" => {
			builder.add(FontSize);
		}
		"lineHeight" => {
			builder.add(LineHeight);
		}
		"textAlign" => {
			builder.add(TextAlign::default());
		}
		"typography" => {
			builder.add(Typography::default());
		}
		"characterCount" => {
			builder.add(CharacterCount::default());
		}
		"placeholder" => {
			builder.add(Placeholder::default());
		}
		"mention" => {
			builder.add(Mention::default());
		}
		"hashtag" => {
			builder.add(Hashtag);
		}
		"emoji" => {
			builder.add(Emoji);
		}
		"image" => {
			builder.add(Image);
		}
		"youtube" => {
			builder.add(Youtube);
		}
		"twitch" => {
			builder.add(Twitch);
		}
		"audio" => {
			builder.add(Audio);
		}
		"file" => {
			builder.add(File);
		}
		"details" => {
			builder.add(Details);
		}
		"callout" => {
			builder.add(Callout);
		}
		"mathematics" => {
			builder.add(Mathematics);
		}
		"table" => {
			builder.add(Table);
		}
		_ => unreachable!("`Extensions::parse` admits only known names"),
	}
}

#[cfg(not(feature = "extras"))]
fn register_extra(_builder: &mut EditorBuilder, _name: &str) {
	unreachable!("`Extensions::parse` admits only known names");
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
