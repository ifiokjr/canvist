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
//! Beyond the StarterKit, each behind its own feature (`full` enables
//! them all, and [`extras`] lists them):
//!
//! | Extension           | Feature            | Adds                                     |
//! | ------------------- | ------------------ | ---------------------------------------- |
//! | [`Subscript`]       | `subscript`        | `subscript`, `Mod-,`                     |
//! | [`Superscript`]     | `superscript`      | `superscript`, `Mod-.`                   |
//! | [`Highlight`]       | `highlight`        | `highlight`, `==text==`, `Mod-Shift-h`   |
//! | [`TextStyle`]       | `text-style`       | `textStyle`, the mark styles live on     |
//! | [`Color`]           | `color`            | `textStyle.color`                        |
//! | [`BackgroundColor`] | `background-color` | `textStyle.backgroundColor`              |
//! | [`FontFamily`]      | `font-family`      | `textStyle.fontFamily`                   |
//! | [`FontSize`]        | `font-size`        | `textStyle.fontSize`                     |
//! | [`LineHeight`]      | `line-height`      | `textStyle.lineHeight`                   |
//! | [`TextAlign`]       | `text-align`       | `textAlign`, `Mod-Shift-l`/`e`/`r`/`j`   |
//! | [`Typography`]      | `typography`       | Smart quotes, dashes, arrows, fractions  |
//! | [`CharacterCount`]  | `character-count`  | Character and word counts, a limit       |
//! | [`Placeholder`]     | `placeholder`      | Hint text in empty blocks                |
//! | [`Mention`]         | `mention`          | `mention`, `@` suggestions               |
//! | [`Hashtag`]         | `hashtag`          | `hashtag`, `#tag` as you type            |
//! | [`Emoji`]           | `emoji`            | `emoji`, `:smile:`, `:` suggestions      |
//! | [`Image`]           | `image`            | `image`, `![alt](src)`                   |
//! | [`Youtube`]         | `youtube`          | `youtube`                                |
//! | [`Twitch`]          | `twitch`           | `twitch`                                 |
//! | [`Audio`]           | `audio`            | `audio`                                  |
//! | [`File`]            | `file`             | `file`, an attachment card               |
//!
//! `docs/src/extensions/parity.md` compares the whole set with Remirror and
//! TipTap.
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
#[cfg(feature = "character-count")]
mod character_count;
#[cfg(feature = "clipboard")]
pub mod clipboard;
#[cfg(feature = "code-block")]
mod code_block;
#[cfg(feature = "emoji")]
pub mod emoji;
#[cfg(feature = "extras")]
mod extras;
#[cfg(feature = "hashtag")]
mod hashtag;

#[cfg(feature = "hard-break")]
mod hard_break;

#[cfg(feature = "heading")]
mod heading;
#[cfg(feature = "highlight")]
mod highlight;
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
	feature = "code",
	feature = "subscript",
	feature = "superscript"
))]
mod marks;
#[cfg(any(
	feature = "audio",
	feature = "file",
	feature = "image",
	feature = "twitch",
	feature = "youtube"
))]
mod media;
#[cfg(feature = "mention")]
mod mention;
#[cfg(feature = "ordered-list")]
mod ordered_list;
#[cfg(feature = "placeholder")]
mod placeholder;

mod paragraph;
mod resolve;
mod starter_kit;
#[cfg(any(feature = "mention", feature = "hashtag", feature = "emoji"))]
mod suggestion;
#[cfg(feature = "task-list")]
mod task_list;
#[cfg(feature = "text-align")]
mod text_align;
#[cfg(feature = "text-style")]
mod text_style;
#[cfg(feature = "trailing-node")]
mod trailing_node;
#[cfg(feature = "typography")]
mod typography;

#[cfg(feature = "blockquote")]
pub use blockquote::Blockquote;
#[cfg(feature = "blockquote")]
pub use blockquote::BlockquoteHandle;
#[cfg(feature = "bullet-list")]
pub use bullet_list::BulletList;
#[cfg(feature = "bullet-list")]
pub use bullet_list::BulletListHandle;
#[cfg(feature = "character-count")]
pub use character_count::CharacterCount;
#[cfg(feature = "character-count")]
pub use character_count::CharacterCountHandle;
#[cfg(feature = "code-block")]
pub use code_block::CodeBlock;
#[cfg(feature = "code-block")]
pub use code_block::CodeBlockHandle;
#[cfg(feature = "emoji")]
pub use emoji::Emoji;
#[cfg(feature = "emoji")]
pub use emoji::EmojiHandle;
#[cfg(feature = "extras")]
pub use extras::extras;
#[cfg(feature = "hard-break")]
pub use hard_break::HardBreak;
#[cfg(feature = "hard-break")]
pub use hard_break::HardBreakHandle;
#[cfg(feature = "hashtag")]
pub use hashtag::Hashtag;
#[cfg(feature = "hashtag")]
pub use hashtag::HashtagHandle;
#[cfg(feature = "heading")]
pub use heading::Heading;
#[cfg(feature = "heading")]
pub use heading::HeadingHandle;
#[cfg(feature = "heading")]
pub use heading::HeadingLevel;
#[cfg(feature = "highlight")]
pub use highlight::Highlight;
#[cfg(feature = "highlight")]
pub use highlight::HighlightHandle;
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
	feature = "code",
	feature = "subscript",
	feature = "superscript"
))]
pub use marks::MarkHandle;
#[cfg(feature = "strike")]
pub use marks::Strike;
#[cfg(feature = "subscript")]
pub use marks::Subscript;
#[cfg(feature = "superscript")]
pub use marks::Superscript;
#[cfg(feature = "underline")]
pub use marks::Underline;
#[cfg(feature = "audio")]
pub use media::Audio;
#[cfg(feature = "file")]
pub use media::File;
#[cfg(feature = "image")]
pub use media::Image;
#[cfg(any(
	feature = "audio",
	feature = "file",
	feature = "image",
	feature = "twitch",
	feature = "youtube"
))]
pub use media::MediaHandle;
#[cfg(feature = "twitch")]
pub use media::Twitch;
#[cfg(feature = "youtube")]
pub use media::Youtube;
#[cfg(feature = "mention")]
pub use mention::Mention;
#[cfg(feature = "mention")]
pub use mention::MentionHandle;
#[cfg(feature = "mention")]
pub use mention::MentionTarget;
#[cfg(feature = "ordered-list")]
pub use ordered_list::OrderedList;
#[cfg(feature = "ordered-list")]
pub use ordered_list::OrderedListHandle;
pub use paragraph::Document;
pub use paragraph::Paragraph;
pub use paragraph::ParagraphHandle;
#[cfg(feature = "placeholder")]
pub use placeholder::Placeholder;
#[cfg(feature = "placeholder")]
pub use placeholder::PlaceholderHandle;
pub use starter_kit::StarterKit;
pub use starter_kit::StarterKitHandles;
#[cfg(feature = "task-list")]
pub use task_list::TaskList;
#[cfg(feature = "task-list")]
pub use task_list::TaskListHandle;
#[cfg(feature = "text-align")]
pub use text_align::Alignment;
#[cfg(feature = "text-align")]
pub use text_align::TextAlign;
#[cfg(feature = "text-align")]
pub use text_align::TextAlignHandle;
#[cfg(feature = "background-color")]
pub use text_style::BackgroundColor;
#[cfg(feature = "color")]
pub use text_style::Color;
#[cfg(feature = "font-family")]
pub use text_style::FontFamily;
#[cfg(feature = "font-size")]
pub use text_style::FontSize;
#[cfg(feature = "line-height")]
pub use text_style::LineHeight;
#[cfg(feature = "text-style")]
pub use text_style::StyleAttrHandle;
#[cfg(feature = "text-style")]
pub use text_style::TextStyle;
#[cfg(feature = "text-style")]
pub use text_style::TextStyleHandle;
#[cfg(feature = "trailing-node")]
pub use trailing_node::TrailingNode;
#[cfg(feature = "typography")]
pub use typography::Typography;
#[cfg(feature = "typography")]
pub use typography::TypographyHandle;
