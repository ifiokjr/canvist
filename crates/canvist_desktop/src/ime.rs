//! Input methods: preedit text lives in the document while it is composed,
//! underlined, and the platform's candidate window sits under it.

use canvist_headless::canvist_core::Editor;
use canvist_layout::Rect;

use crate::DesktopEditor;
use crate::DesktopResult;
use crate::Ime;
use crate::Response;

impl<Handles: Copy> DesktopEditor<Handles> {
	/// Handle an input method event.
	pub(crate) fn ime(&mut self, ime: &Ime) -> DesktopResult<Response> {
		let changed = match ime {
			Ime::Enabled => false,
			Ime::Preedit(text) if text.is_empty() => self.cancel_composition()?,
			Ime::Preedit(text) => self.edit(|editor, env| editor.set_composition(text, env))?,
			Ime::Commit(text) if self.is_composing() => {
				self.edit(|editor, env| editor.commit_composition(text, env))?
			}
			Ime::Commit(text) => self.type_text(text)?,
			Ime::Disabled => self.cancel_composition()?,
		};

		Ok(Response {
			consumed: !matches!(ime, Ime::Enabled | Ime::Disabled) || changed,
			repaint: changed,
		})
	}

	/// Whether an input method is composing text.
	#[must_use]
	pub fn is_composing(&self) -> bool {
		self.doc.state().composition().is_some()
	}

	/// Where the platform should put the input method's candidate window, in
	/// the editor's logical coordinates: under the text being composed, or at
	/// the caret.
	#[must_use]
	pub fn ime_area(&self) -> Option<Rect> {
		let Some(composition) = self.doc.state().composition() else {
			return self.caret_area();
		};
		let range = canvist_headless::canvist_model::Selection::text(
			composition.start_point(),
			composition.end_point(),
		);
		let rects = self.layout.selection_rects(&range);
		let first = rects.first().copied().or_else(|| self.caret_area_raw())?;
		Some(self.to_view_rect(first))
	}

	/// The rectangles to underline under the text being composed, in
	/// document coordinates.
	pub(crate) fn composition_rects(&self) -> Vec<Rect> {
		let Some(composition) = self.doc.state().composition() else {
			return Vec::new();
		};
		let range = canvist_headless::canvist_model::Selection::text(
			composition.start_point(),
			composition.end_point(),
		);
		self.layout.selection_rects(&range)
	}

	fn cancel_composition(&mut self) -> DesktopResult<bool> {
		if !self.is_composing() {
			return Ok(false);
		}

		self.edit(Editor::cancel_composition)
	}

	/// The caret rectangle in document coordinates.
	fn caret_area_raw(&self) -> Option<Rect> {
		self.caret().and_then(|caret| self.layout.caret_rect(caret))
	}
}
