//! Keys: clipboard shortcuts, the editor's key bindings, paging, and
//! typing.

use canvist_headless::canvist_core::KeyPress;
use canvist_headless::canvist_core::Platform;
use canvist_headless::canvist_model::Selection;

use crate::Context;
use crate::DesktopEditor;
use crate::DesktopResult;
use crate::KeyInput;
use crate::Modifiers;
use crate::Response;
use crate::clipboard::ClipboardAction;

/// The share of the viewport a page key moves by, so a little of the last
/// page stays in view.
const PAGE_FRACTION: f32 = 0.9;

impl<Handles: Copy> DesktopEditor<Handles> {
	/// Handle a key press: a clipboard shortcut, then the editor's key
	/// bindings, then paging, and finally the text the key types.
	pub(crate) fn key(&mut self, key: &KeyInput, cx: &mut Context<'_>) -> DesktopResult<Response> {
		let platform = self.options.platform;

		if let Some(action) = clipboard_shortcut(key, platform) {
			return self.clipboard(action, cx);
		}

		let press = KeyPress::new(&key.key, key.modifiers);

		if self.edit(|editor, env| editor.handle_key(press, env))? {
			return Ok(Response::CHANGED);
		}

		match key.key.as_str() {
			"PageUp" => return self.page(-1.0, key.modifiers.shift),
			"PageDown" => return self.page(1.0, key.modifiers.shift),
			_ => {}
		}

		match typed_text(key, platform) {
			Some(text) => self.type_text(text).map(Response::changed),
			None => Ok(Response::IGNORED),
		}
	}

	/// Type `text` at the selection, as a keyboard does: input rules apply,
	/// so `"## "` makes a heading.
	pub fn type_text(&mut self, text: &str) -> DesktopResult<bool> {
		self.edit(|editor, env| editor.insert_text(text, env))
	}

	/// Scroll a page in `direction` (-1 up, 1 down) and move the caret by
	/// the same distance, extending the selection with `extend`.
	fn page(&mut self, direction: f32, extend: bool) -> DesktopResult<Response> {
		let Some(area) = self.caret_area() else {
			return Ok(Response::IGNORED);
		};
		let distance = self.viewport.height * PAGE_FRACTION * direction;
		self.scroll_to(self.scroll + distance);
		self.frame(self.now)?;

		let target = (area.x, area.y + area.height / 2.0);
		let (x, y) = self.to_document(target.0, target.1);
		let Some(caret) = self.layout.hit_test(x, y) else {
			return Ok(Response::CHANGED);
		};

		let anchor = match self.doc.selection() {
			Selection::Text { anchor, .. } if extend => anchor,
			_ => caret.point,
		};

		self.select(Selection::text(anchor, caret.point))?;
		Ok(Response::CHANGED)
	}
}

/// The clipboard action a key press asks for: `Mod-c`, `Mod-x`, `Mod-v`,
/// and `Shift-Mod-v` (paste as plain text), the dedicated Copy, Cut, and
/// Paste keys, and on Linux and Windows `Ctrl-Insert`, `Shift-Insert`, and
/// `Shift-Delete`.
fn clipboard_shortcut(key: &KeyInput, platform: Platform) -> Option<ClipboardAction> {
	clipboard_key(&key.key)
		.or_else(|| insert_shortcut(key, platform))
		.or_else(|| mod_shortcut(key, platform))
}

/// The dedicated Copy, Cut, and Paste keys.
fn clipboard_key(key: &str) -> Option<ClipboardAction> {
	match key {
		"Copy" => Some(ClipboardAction::Copy),
		"Cut" => Some(ClipboardAction::Cut),
		"Paste" => Some(ClipboardAction::Paste),
		_ => None,
	}
}

/// `Ctrl-Insert`, `Shift-Insert`, and `Shift-Delete`, which Linux and
/// Windows keep from IBM's Common User Access.
fn insert_shortcut(key: &KeyInput, platform: Platform) -> Option<ClipboardAction> {
	let modifiers = key.modifiers;

	if platform != Platform::Other {
		return None;
	}

	match (key.key.as_str(), modifiers.ctrl, modifiers.shift) {
		("Insert", true, false) => Some(ClipboardAction::Copy),
		("Insert", false, true) => Some(ClipboardAction::Paste),
		("Delete", false, true) => Some(ClipboardAction::Cut),
		_ => None,
	}
}

/// `Mod-c`, `Mod-x`, `Mod-v`, and `Shift-Mod-v`.
fn mod_shortcut(key: &KeyInput, platform: Platform) -> Option<ClipboardAction> {
	let modifiers = key.modifiers;

	if !is_mod(modifiers, platform) || modifiers.alt {
		return None;
	}

	match (key.key.to_ascii_lowercase().as_str(), modifiers.shift) {
		("c", false) => Some(ClipboardAction::Copy),
		("x", false) => Some(ClipboardAction::Cut),
		("v", false) => Some(ClipboardAction::Paste),
		("v", true) => Some(ClipboardAction::PastePlain),
		_ => None,
	}
}

/// Whether exactly the platform's `Mod` key is held: Cmd on Apple, Ctrl
/// elsewhere.
fn is_mod(modifiers: Modifiers, platform: Platform) -> bool {
	match platform {
		Platform::Apple => modifiers.meta && !modifiers.ctrl,
		Platform::Other => modifiers.ctrl && !modifiers.meta,
	}
}

/// The text a key press types, when no binding took it. Shortcuts type
/// nothing: Cmd or Ctrl chords, except Ctrl with Alt, which is how Windows
/// reports `AltGr`. Control characters such as Tab and Return type nothing
/// either; the key bindings handle them.
fn typed_text(key: &KeyInput, platform: Platform) -> Option<&str> {
	let modifiers = key.modifiers;
	let text = key.text.as_deref().filter(|text| !text.is_empty())?;

	let chord = match platform {
		Platform::Apple => modifiers.meta || modifiers.ctrl,
		Platform::Other => modifiers.meta || (modifiers.ctrl && !modifiers.alt),
	};

	if chord || text.chars().any(char::is_control) {
		return None;
	}

	Some(text)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn with(key: &str, modifiers: Modifiers) -> KeyInput {
		KeyInput::text(key, modifiers)
	}

	const CTRL: Modifiers = Modifiers {
		ctrl: true,
		..Modifiers::NONE
	};
	const META: Modifiers = Modifiers {
		meta: true,
		..Modifiers::NONE
	};

	#[test]
	fn mod_is_cmd_on_apple_and_ctrl_elsewhere() {
		assert_eq!(
			clipboard_shortcut(&with("c", META), Platform::Apple),
			Some(ClipboardAction::Copy)
		);
		assert_eq!(clipboard_shortcut(&with("c", CTRL), Platform::Apple), None);
		assert_eq!(
			clipboard_shortcut(&with("c", CTRL), Platform::Other),
			Some(ClipboardAction::Copy)
		);
		assert_eq!(clipboard_shortcut(&with("c", META), Platform::Other), None);
	}

	#[test]
	fn shift_mod_v_pastes_plain_text() {
		let shift_meta = Modifiers {
			shift: true,
			..META
		};

		assert_eq!(
			clipboard_shortcut(&with("V", shift_meta), Platform::Apple),
			Some(ClipboardAction::PastePlain)
		);
	}

	#[test]
	fn altgr_types_but_ctrl_chords_do_not() {
		let altgr = Modifiers { alt: true, ..CTRL };

		assert_eq!(typed_text(&with("@", altgr), Platform::Other), Some("@"));
		assert_eq!(typed_text(&with("s", CTRL), Platform::Other), None);
		assert_eq!(typed_text(&with("s", META), Platform::Apple), None);
		assert_eq!(
			typed_text(&with("\t", Modifiers::NONE), Platform::Apple),
			None
		);
		assert_eq!(
			typed_text(
				&with(
					"∫",
					Modifiers {
						alt: true,
						..Modifiers::NONE
					}
				),
				Platform::Apple
			),
			Some("∫")
		);
	}
}
