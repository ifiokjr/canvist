//! Editing a text frame with the real canvist text editor.
//!
//! A [`TextSession`] borrows a text frame's document from the scene (see
//! `Scene::lend_text`) and hands it to a `canvist_editor::Editor`, so every
//! text extension (marks, lists, links, input rules, undo) works inside the
//! frame. While the session is open the editor owns the document: keys go
//! to it, its commits are the frame's changes, and the canvas lays the frame
//! out from it. Closing the session hands the document back.

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::EditorBuilder;
use canvist_editor::Env;
use canvist_editor::KeyPress;
use canvist_editor::Platform;
use canvist_editor::Update;
use canvist_layout::Caret;
use canvist_model::Document;
use canvist_model::Selection;
use canvist_scene::NodeId;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Rect;
use kurbo::Shape as _;

use crate::CanvasError;
use crate::CanvasState;
use crate::Modifiers;
use crate::OverlayItem;

/// Flattening tolerance for caret and highlight outlines, in pixels.
const TOLERANCE: f64 = 0.1;

/// The canvist text editor, editing one text frame.
pub struct TextSession {
	node: NodeId,
	editor: Editor,
}

impl std::fmt::Debug for TextSession {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("TextSession")
			.field("node", &self.node)
			.finish_non_exhaustive()
	}
}

impl TextSession {
	/// Open text frame `node` in an editor built by `builder`, with the
	/// caret at the end of its text. `builder` must register the extensions
	/// whose schema the scene's text frames use.
	pub fn open(
		state: &mut CanvasState,
		node: NodeId,
		builder: EditorBuilder,
	) -> Result<Self, CanvasError> {
		let doc = state.scene_mut().lend_text(node)?;
		let end = doc.last_text_block().map(|block| caret_at_end(&doc, block));

		match builder.build_with(move |_, _| Ok(doc)) {
			Ok(mut editor) => {
				if let Some(end) = end {
					// Moving the caret on a fresh editor cannot fail.
					let _ = editor.select(end, &mut Env::headless(Duration::ZERO));
				}

				Ok(Self { node, editor })
			}
			Err(error) => {
				Err(CanvasError::InvalidArgs(format!(
					"the text editor could not open the frame: {error}"
				)))
			}
		}
	}

	/// The text frame being edited.
	#[must_use]
	pub fn node(&self) -> NodeId {
		self.node
	}

	/// The editor.
	#[must_use]
	pub fn editor(&self) -> &Editor {
		&self.editor
	}

	/// The editor, for commands and for collaboration bindings that apply a
	/// collaborator's text changes through `Editor::apply_external`. Call
	/// [`TextSession::refresh`] after changing the document.
	pub fn editor_mut(&mut self) -> &mut Editor {
		&mut self.editor
	}

	/// Type `text` at the caret, as a keyboard or IME commit would.
	pub fn type_text(
		&mut self,
		state: &mut CanvasState,
		text: &str,
		now: Duration,
	) -> Result<Option<Update>, CanvasError> {
		let update = self
			.editor
			.insert_text(text, &mut Env::headless(now))
			.map_err(|error| editor_error(&error))?;
		self.refresh(state)?;
		Ok(update)
	}

	/// Press `key` (a DOM `KeyboardEvent.key` value) with `modifiers`.
	/// Returns the update if a binding applied.
	pub fn press(
		&mut self,
		state: &mut CanvasState,
		key: &str,
		modifiers: Modifiers,
		now: Duration,
	) -> Result<Option<Update>, CanvasError> {
		let modifiers = editor_modifiers(modifiers, self.editor.platform());
		let update = self
			.editor
			.handle_key(KeyPress::new(key, modifiers), &mut Env::headless(now))
			.map_err(|error| editor_error(&error))?;
		self.refresh(state)?;
		Ok(update)
	}

	/// Lay the frame out again from the editor's document.
	pub fn refresh(&self, state: &mut CanvasState) -> Result<(), CanvasError> {
		state.refresh_text(self.node, self.editor.state().doc())
	}

	/// The caret and selection highlights, in screen space.
	#[must_use]
	pub fn overlay(&self, state: &CanvasState) -> Vec<OverlayItem> {
		let (Some(frame), Some(geometry)) =
			(state.texts().get(self.node), state.index().get(self.node))
		else {
			return Vec::new();
		};
		let to_screen = state.camera().world_to_screen() * geometry.world;
		let selection = self.editor.state().selection();
		let highlights = frame
			.layout
			.selection_rects(&selection)
			.into_iter()
			.map(|rect| {
				OverlayItem::TextHighlight {
					path: outline(to_screen, rect),
				}
			});
		let caret = match selection {
			Selection::Text { head, .. } => frame.layout.caret_rect(Caret::new(head)),
			Selection::Blocks { .. } => None,
		};

		highlights
			.chain(caret.map(|rect| {
				OverlayItem::Caret {
					path: outline(to_screen, rect),
				}
			}))
			.collect()
	}

	/// Hand the document back to the scene and lay the frame out from it.
	pub fn close(self, state: &mut CanvasState) -> Result<(), CanvasError> {
		let doc = self.editor.into_state().into_doc();
		state.scene_mut().return_text(self.node, doc)?;
		state.refresh_nodes(&[self.node])
	}
}

/// A caret after the last character of `block`.
fn caret_at_end(doc: &Document, block: canvist_model::BlockId) -> Selection {
	let len = doc.inline(block).map_or(0, canvist_model::InlineText::len);
	Selection::caret(canvist_model::Point::new(block, len))
}

/// Canvas modifiers as editor modifiers: the primary key is Cmd on Apple
/// platforms and Ctrl elsewhere.
fn editor_modifiers(modifiers: Modifiers, platform: Platform) -> canvist_editor::Modifiers {
	let apple = platform == Platform::Apple;

	canvist_editor::Modifiers {
		shift: modifiers.shift,
		alt: modifiers.alt,
		ctrl: modifiers.primary && !apple,
		meta: modifiers.primary && apple,
	}
}

/// A layout rectangle in frame coordinates as a screen-space outline.
fn outline(to_screen: Affine, rect: canvist_layout::Rect) -> BezPath {
	let rect = Rect::new(
		f64::from(rect.x),
		f64::from(rect.y),
		f64::from(rect.x + rect.width),
		f64::from(rect.y + rect.height),
	);
	to_screen * rect.to_path(TOLERANCE)
}

fn editor_error(error: &canvist_editor::EditorError) -> CanvasError {
	CanvasError::InvalidArgs(format!("the text editor rejected the edit: {error}"))
}
