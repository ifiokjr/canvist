//! Every extension beyond the StarterKit, for apps that want them all.

use std::sync::Arc;

use canvist_core::Extension;

/// Every extension beyond the StarterKit, each with its default
/// configuration, ready to add after
/// [`StarterKit::extensions`](crate::StarterKit::extensions).
///
/// ```
/// use canvist_core::Editor;
/// use canvist_extensions::StarterKit;
///
/// let editor = Editor::builder()
/// 	.extensions(StarterKit::default().extensions())
/// 	.extensions(canvist_extensions::extras())
/// 	.build()?;
///
/// assert!(editor.schema().mark_type("highlight").is_some());
/// # Ok::<(), canvist_core::EditorError>(())
/// ```
#[must_use]
pub fn extras() -> Vec<Arc<dyn Extension>> {
	vec![
		Arc::new(crate::Subscript),
		Arc::new(crate::Superscript),
		Arc::new(crate::Highlight::default()),
		Arc::new(crate::TextStyle),
		Arc::new(crate::Color),
		Arc::new(crate::BackgroundColor),
		Arc::new(crate::FontFamily),
		Arc::new(crate::FontSize),
		Arc::new(crate::LineHeight),
		Arc::new(crate::TextAlign::default()),
		Arc::new(crate::Typography::default()),
		Arc::new(crate::CharacterCount::default()),
		Arc::new(crate::Placeholder::default()),
		Arc::new(crate::Mention::default()),
		Arc::new(crate::Hashtag),
		Arc::new(crate::Emoji),
		Arc::new(crate::Image),
		Arc::new(crate::Youtube),
		Arc::new(crate::Twitch),
		Arc::new(crate::Audio),
		Arc::new(crate::File),
		Arc::new(crate::Details),
		Arc::new(crate::Callout),
		Arc::new(crate::Mathematics),
		Arc::new(crate::Table),
		Arc::new(crate::FindAndReplace),
		Arc::new(crate::TableOfContents),
		Arc::new(crate::Comments),
		Arc::new(crate::DragHandle),
		Arc::new(crate::ClearFormatting),
		Arc::new(crate::Ink),
		Arc::new(crate::Kinetic),
	]
}
