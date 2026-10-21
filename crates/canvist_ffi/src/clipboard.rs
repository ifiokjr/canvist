//! Copy, cut, and paste, in the formats platform pasteboards carry, and the
//! task checkboxes a tap toggles.

use canvist_headless::canvist_editor::command;
use canvist_headless::canvist_editor::commands;
use canvist_headless::canvist_editor::position;
use canvist_headless::canvist_extensions::clipboard;
use canvist_headless::canvist_model::Point;
use canvist_headless::canvist_model::Selection;
use canvist_headless::canvist_model::Value;
use serde_json::json;

use crate::Editor;
use crate::error::Result;
use crate::session::Session;

/// The selection as the formats a pasteboard holds.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ClipboardContent {
	/// Semantic HTML, for rich-text targets (`public.html`, `text/html`).
	pub html: String,
	/// Plain text, for everything else.
	pub text: String,
}

#[uniffi::export]
impl Editor {
	/// The selection for the pasteboard, or `None` when nothing is selected.
	pub fn copy(&self) -> Result<Option<ClipboardContent>> {
		self.lock().copy()
	}

	/// Copy the selection and delete it.
	pub fn cut(&self) -> Result<Option<ClipboardContent>> {
		self.change(|session| {
			let Some(content) = session.copy()? else {
				return Ok(None);
			};

			session.edit(|editor, env| editor.run(&command(commands::delete_selection), env))?;
			Ok(Some(content))
		})
	}

	/// Paste HTML over the selection; paste rules apply.
	pub fn paste_html(&self, html: &str) -> Result<bool> {
		self.change(|session| {
			let slice = clipboard::from_html(session.doc.document().schema(), html);
			session.edit(|editor, env| editor.paste(slice, env))
		})
	}

	/// Paste Markdown over the selection.
	pub fn paste_markdown(&self, markdown: &str) -> Result<bool> {
		self.change(|session| {
			let slice = clipboard::from_markdown(session.doc.document().schema(), markdown);
			session.edit(|editor, env| editor.paste(slice, env))
		})
	}

	/// Paste plain text over the selection: one paragraph per line, or
	/// verbatim inside code.
	pub fn paste_text(&self, text: &str) -> Result<bool> {
		self.change(|session| session.edit(|editor, env| editor.paste_text(text, env)))
	}

	/// Toggle the checkbox of the task whose first line is at `(x, y)`, in
	/// document coordinates, as a tap on a checkbox item does, keeping the
	/// selection where it was. Returns whether there was a task there.
	pub fn toggle_task_at(&self, x: f32, y: f32) -> Result<bool> {
		self.change(|session| session.toggle_task_at(x, y))
	}
}

impl Session {
	fn copy(&self) -> Result<Option<ClipboardContent>> {
		let doc = self.doc.document();
		let collapsed =
			position::text_range(doc, self.doc.selection()).is_none_or(|range| range.is_empty());

		if collapsed {
			return Ok(None);
		}

		let slice = self
			.doc
			.editor()
			.copy()
			.map_err(canvist_headless::HeadlessError::from)?;
		let schema = doc.schema();

		Ok(Some(ClipboardContent {
			html: clipboard::to_html(schema, &slice),
			text: clipboard::to_plain_text(schema, &slice),
		}))
	}

	fn toggle_task_at(&mut self, x: f32, y: f32) -> Result<bool> {
		let Some((block, checked)) = self.task_at(x, y) else {
			return Ok(false);
		};
		let before = self.doc.selection();
		let caret = Selection::caret(Point::new(block, 0));

		self.edit(|editor, env| editor.select(caret, env))?;
		let command = self
			.doc
			.command_named("setTaskChecked", &json!({ "checked": !checked }))?;
		let toggled = self.edit(|editor, env| editor.run(&command, env));
		self.edit(|editor, env| editor.select(before, env))?;
		toggled
	}

	/// The first text block of the task item at `(x, y)`, and whether the
	/// task is done.
	fn task_at(&self, x: f32, y: f32) -> Option<(canvist_headless::canvist_model::BlockId, bool)> {
		let doc = self.doc.document();
		let block = self.layout.hit_block(x, y)?;
		let item = doc.block(block)?.parent()?;
		let checked = doc
			.block(item)?
			.attrs()
			.get("checked")
			.and_then(Value::as_bool)?;
		let first = *doc.children(Some(item)).first()?;

		(first == block).then_some((block, checked))
	}
}
