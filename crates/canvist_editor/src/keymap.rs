//! Key bindings with platform-aware modifiers.
//!
//! Bindings are written the way ProseMirror and TipTap write them:
//! `Mod-Shift-z`, `Alt-ArrowLeft`, `Enter`. Key names are DOM
//! `KeyboardEvent.key` values, which every host can produce. `Mod` means
//! Cmd on Apple platforms and Ctrl elsewhere.

use rustc_hash::FxHashMap;

use crate::Command;
use crate::EditorError;

/// The platform family, which decides what `Mod` means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Platform {
	/// macOS, iOS, and iPadOS: `Mod` is Cmd.
	Apple,
	/// Everything else: `Mod` is Ctrl.
	#[default]
	Other,
}

/// The modifier keys held during a key press: the four flags that
/// key events on every platform carry.
#[allow(
	clippy::struct_excessive_bools,
	reason = "one flag per physical modifier key"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
	/// Shift.
	pub shift: bool,
	/// Control.
	pub ctrl: bool,
	/// Alt, or Option on Apple keyboards.
	pub alt: bool,
	/// Meta: Cmd on Apple keyboards, the Windows key elsewhere.
	pub meta: bool,
}

impl Modifiers {
	/// No modifiers.
	pub const NONE: Self = Self {
		shift: false,
		ctrl: false,
		alt: false,
		meta: false,
	};
	/// Only Shift.
	pub const SHIFT: Self = Self {
		shift: true,
		..Self::NONE
	};
}

/// A key press as the host reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyPress<'a> {
	/// The DOM `KeyboardEvent.key` value, e.g. `a`, `Enter`, `ArrowLeft`.
	pub key: &'a str,
	/// The modifiers held.
	pub modifiers: Modifiers,
}

impl<'a> KeyPress<'a> {
	/// A key press with modifiers.
	#[must_use]
	pub const fn new(key: &'a str, modifiers: Modifiers) -> Self {
		Self { key, modifiers }
	}

	/// A key press without modifiers.
	#[must_use]
	pub const fn plain(key: &'a str) -> Self {
		Self::new(key, Modifiers::NONE)
	}
}

/// Parse a binding such as `Mod-Shift-z` into its key and modifiers.
pub fn parse_binding(
	binding: &str,
	platform: Platform,
) -> Result<(String, Modifiers), EditorError> {
	let invalid = || EditorError::InvalidKey(binding.to_owned());

	let (prefix, key) = match (binding, binding.strip_suffix("--")) {
		("-", _) => (None, "-"),
		(_, Some(prefix)) => (Some(prefix), "-"),
		_ => {
			binding
				.rsplit_once('-')
				.map_or((None, binding), |(prefix, key)| (Some(prefix), key))
		}
	};

	if key.is_empty() {
		return Err(invalid());
	}

	let mut modifiers = Modifiers::NONE;

	for name in prefix.into_iter().flat_map(|prefix| prefix.split('-')) {
		let name = match (name, platform) {
			("Mod", Platform::Apple) => "Meta",
			("Mod", Platform::Other) => "Ctrl",
			(name, _) => name,
		};

		match name {
			"Shift" => modifiers.shift = true,
			"Ctrl" | "Control" => modifiers.ctrl = true,
			"Alt" | "Option" => modifiers.alt = true,
			"Meta" | "Cmd" => modifiers.meta = true,
			_ => return Err(invalid()),
		}
	}

	Ok((normalize(key).into_owned(), modifiers))
}

/// Single characters match case-insensitively, since Shift already says
/// whether the key was shifted.
fn normalize(key: &str) -> std::borrow::Cow<'_, str> {
	let mut chars = key.chars();

	match (chars.next(), chars.next()) {
		(Some(only), None) if only.is_uppercase() => only.to_lowercase().collect::<String>().into(),
		_ => key.into(),
	}
}

/// Commands bound to key presses. Several commands may share a key; they run
/// in binding order until one applies.
#[derive(Clone, Default)]
pub struct Keymap {
	bindings: FxHashMap<Modifiers, FxHashMap<Box<str>, Vec<Command>>>,
}

impl Keymap {
	/// An empty keymap.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Bind `command` to `binding`, after any commands already bound to it.
	pub fn bind(
		&mut self,
		binding: &str,
		platform: Platform,
		command: Command,
	) -> Result<(), EditorError> {
		let (key, modifiers) = parse_binding(binding, platform)?;
		self.bindings
			.entry(modifiers)
			.or_default()
			.entry(key.into_boxed_str())
			.or_default()
			.push(command);
		Ok(())
	}

	/// The commands bound to `press`, in the order to try them. A shifted
	/// character with no binding of its own falls back to its unshifted
	/// binding, so `?` bound without Shift still matches Shift-`/`.
	#[must_use]
	pub fn lookup(&self, press: KeyPress<'_>) -> &[Command] {
		let key = normalize(press.key);
		let found = self.find(press.modifiers, &key);

		if !found.is_empty() || !press.modifiers.shift || key.chars().nth(1).is_some() {
			return found;
		}

		let unshifted = Modifiers {
			shift: false,
			..press.modifiers
		};
		self.find(unshifted, &key)
	}

	fn find(&self, modifiers: Modifiers, key: &str) -> &[Command] {
		self.bindings
			.get(&modifiers)
			.and_then(|keys| keys.get(key))
			.map_or(&[], Vec::as_slice)
	}
}

impl std::fmt::Debug for Keymap {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let count: usize = self.bindings.values().map(FxHashMap::len).sum();
		formatter
			.debug_struct("Keymap")
			.field("keys", &count)
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn mod_follows_the_platform() {
		let apple = parse_binding("Mod-Shift-Z", Platform::Apple).expect("valid");
		let other = parse_binding("Mod-b", Platform::Other).expect("valid");

		assert_eq!(
			apple,
			(
				"z".to_owned(),
				Modifiers {
					shift: true,
					meta: true,
					..Modifiers::NONE
				}
			)
		);
		assert_eq!(
			other,
			(
				"b".to_owned(),
				Modifiers {
					ctrl: true,
					..Modifiers::NONE
				}
			)
		);
	}

	#[test]
	fn parses_named_keys_and_the_minus_key() {
		let (key, modifiers) = parse_binding("Alt-ArrowLeft", Platform::Other).expect("valid");
		let (minus, ctrl) = parse_binding("Ctrl--", Platform::Other).expect("valid");

		assert_eq!(key, "ArrowLeft");
		assert!(modifiers.alt);
		assert_eq!(minus, "-");
		assert!(ctrl.ctrl);
	}

	#[test]
	fn rejects_unknown_modifiers_and_empty_keys() {
		assert!(parse_binding("Hyper-a", Platform::Other).is_err());
		assert!(parse_binding("Ctrl-", Platform::Other).is_err());
	}
}
