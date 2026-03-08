//! Platform-agnostic editor input event model.
//!
//! This module defines a canonical stream of editor intent events that can be
//! produced by any input backend (web, mobile, desktop) before they are
//! converted into document operations.

use crate::Position;
use crate::Selection;

/// Modifier state attached to keyboard and pointer events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Modifiers {
	/// Whether the Shift key is held.
	pub shift: bool,
	/// Whether the Control key is held.
	pub control: bool,
	/// Whether the Alt/Option key is held.
	pub alt: bool,
	/// Whether the Meta/Command/Windows key is held.
	pub meta: bool,
}

/// Logical keyboard key representation used across platforms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorKey {
	Character(String),
	Enter,
	Tab,
	Backspace,
	Delete,
	Escape,
	ArrowLeft,
	ArrowRight,
	ArrowUp,
	ArrowDown,
	Home,
	End,
	PageUp,
	PageDown,
	Unknown(String),
}

/// Pointer/touch contact phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerPhase {
	Down,
	Move,
	Up,
	Cancel,
}

/// Normalized pointer/touch event payload.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
	pub phase: PointerPhase,
	pub pointer_id: i64,
	pub x: f64,
	pub y: f64,
	pub pressure: f32,
	pub modifiers: Modifiers,
}

/// Composition (IME) lifecycle for complex text input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionPhase {
	Start,
	Update,
	End,
}

/// Canonical editor event stream.
///
/// Input backends should map platform-native events into this enum before any
/// operation generation occurs.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorEvent {
	TextInsert { text: String },
	TextDeleteBackward { count: usize },
	TextDeleteForward { count: usize },
	KeyDown {
		key: EditorKey,
		modifiers: Modifiers,
		repeat: bool,
	},
	KeyUp {
		key: EditorKey,
		modifiers: Modifiers,
	},
	Pointer(PointerEvent),
	SelectionSet { selection: Selection },
	CursorMove { position: Position, extend: bool },
	Composition {
		phase: CompositionPhase,
		text: String,
	},
	ClipboardCopy,
	ClipboardCut,
	ClipboardPaste { text: String },
	Focus,
	Blur,
}

/// Source of canonical editor events.
///
/// Implementors normalize platform input (DOM, `UIKit`, Android View, desktop
/// windowing systems, etc.) into [`EditorEvent`] values.
pub trait EventSource {
	/// Returns the next pending event, if available.
	fn poll_event(&mut self) -> Option<EditorEvent>;
}
