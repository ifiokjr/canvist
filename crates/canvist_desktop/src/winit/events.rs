//! winit's window events as the editor's [`InputEvent`]s.

use kurbo::Point;
use winit::event::ElementState;
use winit::event::KeyEvent;
use winit::event::MouseButton;
use winit::event::MouseScrollDelta;
use winit::event::WindowEvent;
use winit::keyboard::Key;
use winit::keyboard::ModifiersState;
use winit::keyboard::NamedKey;

use crate::Ime;
use crate::InputEvent;
use crate::KeyInput;
use crate::Modifiers;
use crate::PointerButton;
use crate::ScrollDelta;

/// What translating winit events needs to remember between them: the
/// modifiers held and where the pointer is.
#[derive(Clone, Copy, Debug, Default)]
pub struct WinitInput {
	modifiers: ModifiersState,
	/// The pointer in window logical pixels.
	pointer: Option<Point>,
}

impl WinitInput {
	/// Nothing held, and the pointer outside the window.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// The pointer's last position in window logical pixels.
	#[must_use]
	pub fn pointer(&self) -> Option<Point> {
		self.pointer
	}

	/// The modifiers held now.
	#[must_use]
	pub fn modifiers(&self) -> Modifiers {
		modifiers(self.modifiers)
	}

	/// Translate `event` for an editor whose top-left corner is at `origin`
	/// in window logical pixels, on a window with `scale` physical pixels per
	/// logical pixel. Events the editor has no use for return `None`.
	pub fn translate(
		&mut self,
		event: &WindowEvent,
		origin: Point,
		scale: f64,
	) -> Option<InputEvent> {
		match event {
			WindowEvent::ModifiersChanged(modifiers) => {
				self.modifiers = modifiers.state();
				None
			}
			WindowEvent::KeyboardInput { event, .. } => self.key(event),
			WindowEvent::Ime(ime) => Some(InputEvent::Ime(translate_ime(ime))),
			WindowEvent::CursorMoved { position, .. } => {
				let window = Point::new(position.x / scale, position.y / scale);
				self.pointer = Some(window);
				Some(InputEvent::PointerMove {
					position: relative(window, origin),
				})
			}
			WindowEvent::CursorLeft { .. } => {
				self.pointer = None;
				None
			}
			WindowEvent::MouseInput { state, button, .. } => self.button(*state, *button, origin),
			WindowEvent::MouseWheel { delta, .. } => {
				Some(InputEvent::Scroll(scroll(*delta, scale)))
			}
			WindowEvent::Focused(focused) => Some(InputEvent::Focus(*focused)),
			_ => None,
		}
	}

	fn button(
		&self,
		state: ElementState,
		button: MouseButton,
		origin: Point,
	) -> Option<InputEvent> {
		let button = pointer_button(button);

		if state == ElementState::Released {
			return Some(InputEvent::PointerUp { button });
		}

		Some(InputEvent::PointerDown {
			position: relative(self.pointer?, origin),
			button,
			modifiers: self.modifiers(),
		})
	}

	fn key(&self, event: &KeyEvent) -> Option<InputEvent> {
		if event.state != ElementState::Pressed {
			return None;
		}

		let key = dom_key(&event.logical_key)?;
		Some(InputEvent::Key(KeyInput {
			key,
			text: event.text.as_ref().map(ToString::to_string),
			modifiers: self.modifiers(),
		}))
	}
}

fn relative(point: Point, origin: Point) -> Point {
	Point::new(point.x - origin.x, point.y - origin.y)
}

fn modifiers(state: ModifiersState) -> Modifiers {
	Modifiers {
		shift: state.shift_key(),
		ctrl: state.control_key(),
		alt: state.alt_key(),
		meta: state.super_key(),
	}
}

fn translate_ime(ime: &winit::event::Ime) -> Ime {
	match ime {
		winit::event::Ime::Enabled => Ime::Enabled,
		winit::event::Ime::Preedit(text, _) => Ime::Preedit(text.clone()),
		winit::event::Ime::Commit(text) => Ime::Commit(text.clone()),
		winit::event::Ime::Disabled => Ime::Disabled,
	}
}

fn pointer_button(button: MouseButton) -> PointerButton {
	match button {
		MouseButton::Left => PointerButton::Primary,
		MouseButton::Right => PointerButton::Secondary,
		MouseButton::Middle => PointerButton::Middle,
		_ => PointerButton::Other,
	}
}

/// A wheel delta in the editor's terms: trackpads report physical pixels.
fn scroll(delta: MouseScrollDelta, scale: f64) -> ScrollDelta {
	match delta {
		MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines { x, y },
		MouseScrollDelta::PixelDelta(position) => {
			ScrollDelta::Pixels {
				x: (position.x / scale) as f32,
				y: (position.y / scale) as f32,
			}
		}
	}
}

/// A winit key as DOM `KeyboardEvent.key` names it, as key bindings are
/// written. Keys the editor binds nothing to, such as modifiers alone and
/// function keys, return `None`.
fn dom_key(key: &Key) -> Option<String> {
	let name = match key {
		Key::Character(text) => return Some(text.to_string()),
		Key::Named(NamedKey::Space) => " ",
		Key::Named(named) => named_key(*named)?,
		Key::Unidentified(_) | Key::Dead(_) => return None,
	};

	Some(name.to_owned())
}

fn named_key(key: NamedKey) -> Option<&'static str> {
	Some(match key {
		NamedKey::Enter => "Enter",
		NamedKey::Tab => "Tab",
		NamedKey::Backspace => "Backspace",
		NamedKey::Delete => "Delete",
		NamedKey::Escape => "Escape",
		NamedKey::ArrowLeft => "ArrowLeft",
		NamedKey::ArrowRight => "ArrowRight",
		NamedKey::ArrowUp => "ArrowUp",
		NamedKey::ArrowDown => "ArrowDown",
		NamedKey::Home => "Home",
		NamedKey::End => "End",
		NamedKey::PageUp => "PageUp",
		NamedKey::PageDown => "PageDown",
		NamedKey::Insert => "Insert",
		NamedKey::Copy => "Copy",
		NamedKey::Cut => "Cut",
		NamedKey::Paste => "Paste",
		NamedKey::Undo => "Undo",
		NamedKey::Redo => "Redo",
		_ => return None,
	})
}

#[cfg(test)]
mod tests {
	use winit::keyboard::SmolStr;

	use super::*;

	#[test]
	fn keys_take_their_dom_names() {
		assert_eq!(
			dom_key(&Key::Named(NamedKey::ArrowLeft)).as_deref(),
			Some("ArrowLeft")
		);
		assert_eq!(dom_key(&Key::Named(NamedKey::Space)).as_deref(), Some(" "));
		assert_eq!(
			dom_key(&Key::Character(SmolStr::new("b"))).as_deref(),
			Some("b")
		);
		assert_eq!(dom_key(&Key::Named(NamedKey::Shift)), None);
	}

	#[test]
	fn super_is_meta() {
		let held = modifiers(ModifiersState::SUPER | ModifiersState::SHIFT);

		assert!(held.meta && held.shift && !held.ctrl && !held.alt);
	}

	#[test]
	fn trackpad_pixels_become_logical() {
		let retina = 2.0;
		let physical = winit::dpi::PhysicalPosition::new(0.0, 40.0);
		let delta = MouseScrollDelta::PixelDelta(physical);

		assert_eq!(
			scroll(delta, retina),
			ScrollDelta::Pixels {
				x: 0.0,
				y: (physical.y / retina) as f32,
			}
		);
	}
}
