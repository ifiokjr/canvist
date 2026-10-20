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
use kurbo::Point;
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
	/// whose schema the scene's text frames use. If the editor cannot be
	/// built, the frame keeps its text and the error is returned.
	pub fn open(
		state: &mut CanvasState,
		node: NodeId,
		builder: EditorBuilder,
	) -> Result<Self, CanvasError> {
		let doc = state.scene_mut().lend_text(node)?;
		let end = doc.last_text_block().map(|block| caret_at_end(&doc, block));
		// The editor takes a copy, so a failure can hand the original back.
		let copy = doc.clone();

		let mut editor = match builder.build_with(move |_, _| Ok(copy)) {
			Ok(editor) => editor,
			Err(error) => {
				state.scene_mut().return_text(node, doc)?;
				return Err(open_error(&error));
			}
		};

		if let Some(end) = end
			&& let Err(error) = editor.select(end, &mut Env::headless(Duration::ZERO))
		{
			state.scene_mut().return_text(node, doc)?;
			return Err(open_error(&error));
		}

		Ok(Self { node, editor })
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

	/// Whether screen point `at` falls inside the frame being edited.
	#[must_use]
	pub fn contains(&self, state: &CanvasState, at: Point) -> bool {
		let (Some(point), Some(geometry)) =
			(self.frame_point(state, at), state.index().get(self.node))
		else {
			return false;
		};

		Rect::from_origin_size(Point::ORIGIN, geometry.size.to_size()).contains(point)
	}

	/// Put the caret nearest screen point `at`, extending the selection
	/// from its anchor when `extend` is set (a Shift-click).
	pub fn click(
		&mut self,
		state: &CanvasState,
		at: Point,
		extend: bool,
		now: Duration,
	) -> Result<Option<Update>, CanvasError> {
		let (Some(point), Some(frame)) =
			(self.frame_point(state, at), state.texts().get(self.node))
		else {
			return Ok(None);
		};
		// Layout works in `f32`; frame coordinates fit comfortably.
		#[allow(clippy::cast_possible_truncation)]
		let Some(caret) = frame.layout.hit_test(point.x as f32, point.y as f32) else {
			return Ok(None);
		};

		let selection = match self.editor.state().selection() {
			Selection::Text { anchor, .. } if extend => Selection::text(anchor, caret.point),
			_ => Selection::caret(caret.point),
		};

		self.editor
			.select(selection, &mut Env::headless(now))
			.map_err(|error| editor_error(&error))
	}

	/// Screen point `at` in the frame's own coordinates.
	fn frame_point(&self, state: &CanvasState, at: Point) -> Option<Point> {
		let geometry = state.index().get(self.node)?;
		let to_screen = state.camera().world_to_screen() * geometry.world;
		(to_screen.determinant().abs() > f64::EPSILON).then(|| to_screen.inverse() * at)
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

fn open_error(error: &canvist_editor::EditorError) -> CanvasError {
	CanvasError::InvalidArgs(format!("the text editor could not open the frame: {error}"))
}
