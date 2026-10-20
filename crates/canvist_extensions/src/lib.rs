#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extensions`
//!
//! The canvist StarterKit: the extensions TipTap's StarterKit ships, built
//! on [`canvist_editor`] and named the way TipTap names them, so documents
//! move between the two as ProseMirror JSON without conversion.
//!
//! Each extension sits behind a Cargo feature of the same name, and the
//! `starter-kit` feature (on by default) enables them all. An app that only
//! needs bold paragraphs compiles only bold paragraphs.
//!
//! | Extension          | Feature           | Adds                                    |
//! | ------------------ | ----------------- | --------------------------------------- |
//! | [`Document`]       | always            | The root                                |
//! | [`Paragraph`]      | always            | `paragraph`, `Mod-Alt-0`                |
//! | [`Heading`]        | `heading`         | `heading`, `#` to `######`, `Mod-Alt-n` |
//! | [`Blockquote`]     | `blockquote`      | `blockquote`, `> `, `Mod-Shift-b`       |
//! | [`CodeBlock`]      | `code-block`      | `codeBlock`, ```` ``` ````, `Mod-Alt-c` |
//! | [`HardBreak`]      | `hard-break`      | `hardBreak`, `Shift-Enter`              |
//! | [`HorizontalRule`] | `horizontal-rule` | `horizontalRule`, `---`                 |
//! | [`BulletList`]     | `bullet-list`     | `bulletList`, `- `, `Mod-Shift-8`       |
//! | [`OrderedList`]    | `ordered-list`    | `orderedList`, `1. `, `Mod-Shift-7`     |
//! | [`ListItem`]       | `list-item`       | `listItem`, Enter/Tab/Shift-Tab         |
//! | [`TaskList`]       | `task-list`       | `taskList`, `taskItem`, `[ ] `          |
//! | [`Bold`]           | `bold`            | `bold`, `**text**`, `Mod-b`             |
//! | [`Italic`]         | `italic`          | `italic`, `*text*`, `Mod-i`             |
//! | [`Underline`]      | `underline`       | `underline`, `Mod-u`                    |
//! | [`Strike`]         | `strike`          | `strike`, `~~text~~`, `Mod-Shift-s`     |
//! | [`Code`]           | `code`            | `code`, `` `text` ``, `Mod-e`           |
//! | [`Link`]           | `link`            | `link`, autolink, link on paste         |
//! | [`History`]        | `history`         | `Mod-z`, `Mod-Shift-z`, `Mod-y`         |
//! | [`TrailingNode`]   | `trailing-node`   | A trailing paragraph                    |
//!
//! # Example
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_editor::Editor;
//! use canvist_editor::Env;
//! use canvist_extensions::StarterKit;
//!
//! let mut editor = Editor::builder()
//! 	.extensions(StarterKit::default().extensions())
//! 	.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for text in ["#", " ", "H", "i"] {
//! 	editor.insert_text(text, &mut env)?;
//! }
//!
//! let heading = editor.schema().block_type("heading");
//! let first = editor.state().doc().first_text_block().and_then(|id| editor.state().doc().block(id));
//! assert_eq!(first.map(|block| block.kind()), heading);
//! # Ok::<(), canvist_editor::EditorError>(())
//! ```

#[cfg(feature = "blockquote")]
mod blockquote;
#[cfg(feature = "bullet-list")]
mod bullet_list;
#[cfg(feature = "clipboard")]
pub mod clipboard;
#[cfg(feature = "code-block")]
mod code_block;

#[cfg(feature = "hard-break")]
mod hard_break;

#[cfg(feature = "heading")]
mod heading;
#[cfg(feature = "history")]
mod history;
#[cfg(feature = "horizontal-rule")]
mod horizontal_rule;
#[cfg(feature = "link")]
mod link;

#[cfg(feature = "list-item")]
mod list_item;
#[cfg(any(feature = "list-item", feature = "task-list"))]
mod lists;
#[cfg(any(
	feature = "bold",
	feature = "italic",
	feature = "underline",
	feature = "strike",
	feature = "code"
))]
mod marks;
#[cfg(feature = "ordered-list")]
mod ordered_list;

mod paragraph;
mod resolve;
mod starter_kit;
#[cfg(feature = "task-list")]
mod task_list;
#[cfg(feature = "trailing-node")]
mod trailing_node;

#[cfg(feature = "blockquote")]
pub use blockquote::Blockquote;
#[cfg(feature = "blockquote")]
pub use blockquote::BlockquoteHandle;
#[cfg(feature = "bullet-list")]
pub use bullet_list::BulletList;
#[cfg(feature = "bullet-list")]
pub use bullet_list::BulletListHandle;
#[cfg(feature = "code-block")]
pub use code_block::CodeBlock;
#[cfg(feature = "code-block")]
pub use code_block::CodeBlockHandle;
#[cfg(feature = "hard-break")]
pub use hard_break::HardBreak;
#[cfg(feature = "hard-break")]
pub use hard_break::HardBreakHandle;
#[cfg(feature = "heading")]
pub use heading::Heading;
#[cfg(feature = "heading")]
pub use heading::HeadingHandle;
#[cfg(feature = "heading")]
pub use heading::HeadingLevel;
#[cfg(feature = "history")]
pub use history::History;
#[cfg(feature = "history")]
pub use history::HistoryHandle;
#[cfg(feature = "horizontal-rule")]
pub use horizontal_rule::HorizontalRule;
#[cfg(feature = "horizontal-rule")]
pub use horizontal_rule::HorizontalRuleHandle;
#[cfg(feature = "link")]
pub use link::Link;
#[cfg(feature = "link")]
pub use link::LinkHandle;
#[cfg(feature = "list-item")]
pub use list_item::ListItem;
#[cfg(feature = "list-item")]
pub use list_item::ListItemHandle;
#[cfg(feature = "bold")]
pub use marks::Bold;
#[cfg(feature = "code")]
pub use marks::Code;
#[cfg(feature = "italic")]
pub use marks::Italic;
#[cfg(any(
	feature = "bold",
	feature = "italic",
	feature = "underline",
	feature = "strike",
	feature = "code"
))]
pub use marks::MarkHandle;
#[cfg(feature = "strike")]
pub use marks::Strike;
#[cfg(feature = "underline")]
pub use marks::Underline;
#[cfg(feature = "ordered-list")]
pub use ordered_list::OrderedList;
#[cfg(feature = "ordered-list")]
pub use ordered_list::OrderedListHandle;
pub use paragraph::Document;
pub use paragraph::Paragraph;
pub use paragraph::ParagraphHandle;
pub use starter_kit::StarterKit;
pub use starter_kit::StarterKitHandles;
#[cfg(feature = "task-list")]
pub use task_list::TaskList;
#[cfg(feature = "task-list")]
pub use task_list::TaskListHandle;
#[cfg(feature = "trailing-node")]
pub use trailing_node::TrailingNode;
