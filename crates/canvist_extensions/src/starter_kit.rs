//! The StarterKit: TipTap's starter set in one value.

use std::sync::Arc;

use canvist_editor::BaseKeymap;
use canvist_editor::Extension;
use canvist_editor::UndoInputRule;

use crate::Document;
use crate::Paragraph;

/// TipTap's StarterKit: the document, paragraphs, the standard keys, and
/// Backspace undoing input rules, plus every extension below that is
/// enabled. Turn one off by setting its
/// field to `None` or `false`; leave its Cargo feature off to not compile it
/// at all.
///
/// ```
/// use canvist_editor::Editor;
/// use canvist_extensions::StarterKit;
///
/// let kit = StarterKit {
/// 	horizontal_rule: false,
/// 	..StarterKit::default()
/// };
/// let editor = Editor::builder().extensions(kit.extensions()).build()?;
///
/// assert!(editor.schema().block_type("horizontalRule").is_none());
/// # Ok::<(), canvist_editor::EditorError>(())
/// ```
#[derive(Clone, Debug)]
#[allow(
	clippy::struct_excessive_bools,
	reason = "one switch per optional extension"
)]
pub struct StarterKit {
	/// Headings, with the deepest level offered.
	#[cfg(feature = "heading")]
	pub heading: Option<crate::Heading>,
	/// Block quotes.
	#[cfg(feature = "blockquote")]
	pub blockquote: bool,
	/// Code blocks.
	#[cfg(feature = "code-block")]
	pub code_block: bool,
	/// Hard line breaks.
	#[cfg(feature = "hard-break")]
	pub hard_break: bool,
	/// Horizontal rules.
	#[cfg(feature = "horizontal-rule")]
	pub horizontal_rule: bool,
	/// Bullet lists (with list items).
	#[cfg(feature = "bullet-list")]
	pub bullet_list: bool,
	/// Ordered lists (with list items).
	#[cfg(feature = "ordered-list")]
	pub ordered_list: bool,
	/// Task lists and task items.
	#[cfg(feature = "task-list")]
	pub task_list: bool,
	/// Bold.
	#[cfg(feature = "bold")]
	pub bold: bool,
	/// Italic.
	#[cfg(feature = "italic")]
	pub italic: bool,
	/// Underline.
	#[cfg(feature = "underline")]
	pub underline: bool,
	/// Strikethrough.
	#[cfg(feature = "strike")]
	pub strike: bool,
	/// Inline code, which excludes the other marks the kit registers.
	#[cfg(feature = "code")]
	pub code: bool,
	/// Links, with autolinking and linking on paste.
	#[cfg(feature = "link")]
	pub link: Option<crate::Link>,
	/// Undo and redo keys.
	#[cfg(feature = "history")]
	pub history: bool,
	/// A trailing paragraph.
	#[cfg(feature = "trailing-node")]
	pub trailing_node: bool,
}

#[allow(
	clippy::derivable_impls,
	reason = "derivable only when every extension is compiled out"
)]
impl Default for StarterKit {
	fn default() -> Self {
		Self {
			#[cfg(feature = "heading")]
			heading: Some(crate::Heading::default()),
			#[cfg(feature = "blockquote")]
			blockquote: true,
			#[cfg(feature = "code-block")]
			code_block: true,
			#[cfg(feature = "hard-break")]
			hard_break: true,
			#[cfg(feature = "horizontal-rule")]
			horizontal_rule: true,
			#[cfg(feature = "bullet-list")]
			bullet_list: true,
			#[cfg(feature = "ordered-list")]
			ordered_list: true,
			#[cfg(feature = "task-list")]
			task_list: true,
			#[cfg(feature = "bold")]
			bold: true,
			#[cfg(feature = "italic")]
			italic: true,
			#[cfg(feature = "underline")]
			underline: true,
			#[cfg(feature = "strike")]
			strike: true,
			#[cfg(feature = "code")]
			code: true,
			#[cfg(feature = "link")]
			link: Some(crate::Link::default()),
			#[cfg(feature = "history")]
			history: true,
			#[cfg(feature = "trailing-node")]
			trailing_node: true,
		}
	}
}

/// Collects enabled extensions.
#[derive(Default)]
struct Kit {
	extensions: Vec<Arc<dyn Extension>>,
	marks: Vec<&'static str>,
}

impl Kit {
	fn add(&mut self, enabled: bool, extension: impl Extension + 'static) {
		if enabled {
			self.extensions.push(Arc::new(extension));
		}
	}

	#[cfg_attr(
		not(any(
			feature = "bold",
			feature = "italic",
			feature = "underline",
			feature = "strike",
			feature = "link",
			feature = "code"
		)),
		allow(dead_code)
	)]
	fn add_mark(&mut self, enabled: bool, extension: impl Extension + 'static) {
		if enabled {
			self.marks.push(extension.name());
			self.add(true, extension);
		}
	}
}

impl StarterKit {
	/// The enabled extensions, ready for
	/// [`EditorBuilder::extensions`](canvist_editor::EditorBuilder::extensions).
	#[must_use]
	pub fn extensions(&self) -> Vec<Arc<dyn Extension>> {
		let mut kit = Kit::default();
		kit.add(true, Document);
		kit.add(true, Paragraph);
		kit.add(true, BaseKeymap);
		kit.add(true, UndoInputRule);

		#[cfg(feature = "heading")]
		if let Some(heading) = self.heading {
			kit.add(true, heading);
		}

		#[cfg(feature = "blockquote")]
		kit.add(self.blockquote, crate::Blockquote);
		#[cfg(feature = "code-block")]
		kit.add(self.code_block, crate::CodeBlock);
		#[cfg(feature = "hard-break")]
		kit.add(self.hard_break, crate::HardBreak);
		#[cfg(feature = "horizontal-rule")]
		kit.add(self.horizontal_rule, crate::HorizontalRule);

		#[cfg(feature = "bullet-list")]
		kit.add(self.bullet_list, crate::BulletList);
		#[cfg(feature = "ordered-list")]
		kit.add(self.ordered_list, crate::OrderedList);
		#[cfg(feature = "list-item")]
		kit.add(self.needs_list_item(), crate::ListItem);
		#[cfg(feature = "task-list")]
		kit.add(self.task_list, crate::TaskList);

		#[cfg(feature = "bold")]
		kit.add_mark(self.bold, crate::Bold);
		#[cfg(feature = "italic")]
		kit.add_mark(self.italic, crate::Italic);
		#[cfg(feature = "underline")]
		kit.add_mark(self.underline, crate::Underline);
		#[cfg(feature = "strike")]
		kit.add_mark(self.strike, crate::Strike);

		#[cfg(feature = "link")]
		if let Some(link) = self.link {
			kit.add_mark(true, link);
		}

		#[cfg(feature = "code")]
		{
			let excludes = kit.marks.clone();
			kit.add_mark(self.code, crate::Code { excludes });
		}

		#[cfg(feature = "history")]
		kit.add(self.history, crate::History);
		#[cfg(feature = "trailing-node")]
		kit.add(self.trailing_node, crate::TrailingNode);

		kit.extensions
	}

	/// Bullet and ordered lists share the list item.
	#[cfg(feature = "list-item")]
	#[allow(clippy::unused_self, reason = "both list fields may be compiled out")]
	fn needs_list_item(&self) -> bool {
		#[cfg(feature = "bullet-list")]
		if self.bullet_list {
			return true;
		}

		#[cfg(feature = "ordered-list")]
		if self.ordered_list {
			return true;
		}

		false
	}
}
