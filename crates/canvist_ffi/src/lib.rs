#![doc(
	html_logo_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_ffi`
//!
//! The native binding surface for canvist, exported through
//! [UniFFI](https://mozilla.github.io/uniffi-rs/) to Swift (iOS) and Kotlin
//! (Android), and wrapped by the Flutter bridge for Dart.
//!
//! **This is a placeholder.** [`EditorHandle`] is backed by a plain [`String`]
//! so the iOS, Android and Flutter build pipelines can be proven end to end
//! before the v2 editor engine exists. When the engine lands, the handle will
//! wrap it instead, and this surface will grow into the real binding. Do not
//! build product features against the placeholder behaviour.
//!
//! ```
//! use canvist_ffi::EditorHandle;
//!
//! let editor = EditorHandle::new();
//! editor.insert_text(0, "world")?;
//! editor.insert_text(0, "hello ")?;
//! assert_eq!(editor.text(), "hello world");
//! # Ok::<(), canvist_ffi::EditorError>(())
//! ```

use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

uniffi::setup_scaffolding!();

/// Errors returned across the FFI boundary.
#[derive(Debug, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum EditorError {
	/// The insertion offset is past the end of the text.
	#[error("offset {offset} is out of bounds for text of length {length}")]
	OffsetOutOfBounds {
		/// The requested offset, in Unicode scalar values.
		offset: u32,
		/// The current text length, in Unicode scalar values.
		length: u32,
	},
}

/// A handle to one editor instance, shared with the host platform.
///
/// Foreign code owns the handle through a reference count, so every method
/// takes `&self` and the state lives behind a [`Mutex`]. The handle is safe to
/// use from any thread.
///
/// Placeholder: the state is a plain [`String`] until the v2 engine replaces
/// it.
#[derive(Debug, Default, uniffi::Object)]
pub struct EditorHandle {
	text: Mutex<String>,
}

#[uniffi::export]
impl EditorHandle {
	/// Create an editor with empty text.
	#[uniffi::constructor]
	pub fn new() -> Self {
		Self::default()
	}

	/// Insert `text` before the Unicode scalar value at `offset`.
	///
	/// Offsets count Unicode scalar values (Rust `char`s), not bytes, UTF-16
	/// code units or grapheme clusters. The real engine will define its own
	/// position model; this one only has to be unambiguous on every platform.
	pub fn insert_text(&self, offset: u32, text: &str) -> Result<(), EditorError> {
		let mut current = self.lock();
		let length = current.chars().count();
		let byte_index = if offset as usize == length {
			current.len()
		} else {
			current
				.char_indices()
				.nth(offset as usize)
				.map(|(index, _)| index)
				.ok_or(EditorError::OffsetOutOfBounds {
					offset,
					length: length as u32,
				})?
		};

		current.insert_str(byte_index, text);
		Ok(())
	}

	/// The full text of the document.
	pub fn text(&self) -> String {
		self.lock().clone()
	}

	/// The version of the Rust core, so hosts can check they loaded the
	/// library they were built against.
	pub fn version(&self) -> String {
		env!("CARGO_PKG_VERSION").to_owned()
	}
}

impl EditorHandle {
	/// Every mutation leaves the `String` valid, so a panic on another thread
	/// cannot leave the text half-written and a poisoned lock is still usable.
	fn lock(&self) -> MutexGuard<'_, String> {
		self.text.lock().unwrap_or_else(PoisonError::into_inner)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn starts_empty() {
		assert_eq!(EditorHandle::new().text(), "");
	}

	#[test]
	fn inserts_at_start_middle_and_end() -> Result<(), EditorError> {
		let editor = EditorHandle::new();
		editor.insert_text(0, "ac")?;
		editor.insert_text(1, "b")?;
		editor.insert_text(3, "d")?;
		assert_eq!(editor.text(), "abcd");
		Ok(())
	}

	#[test]
	fn offsets_count_unicode_scalar_values() -> Result<(), EditorError> {
		let editor = EditorHandle::new();
		editor.insert_text(0, "héllo")?;
		editor.insert_text(2, "—")?;
		assert_eq!(editor.text(), "hé—llo");
		Ok(())
	}

	#[test]
	fn rejects_offsets_past_the_end() {
		let editor = EditorHandle::new();
		let result = editor.insert_text(1, "x");
		assert_eq!(
			result,
			Err(EditorError::OffsetOutOfBounds {
				offset: 1,
				length: 0,
			})
		);
		assert_eq!(editor.text(), "");
	}

	#[test]
	fn reports_the_crate_version() {
		assert_eq!(EditorHandle::new().version(), env!("CARGO_PKG_VERSION"));
	}
}
