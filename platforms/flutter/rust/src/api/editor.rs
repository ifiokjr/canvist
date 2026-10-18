//! Dart-facing editor API. Placeholder: mirrors [`canvist_ffi::EditorHandle`]
//! until the v2 engine replaces it.

use flutter_rust_bridge::frb;

/// A handle to one Rust-owned editor instance.
#[frb(opaque)]
pub struct EditorHandle(canvist_ffi::EditorHandle);

impl EditorHandle {
	/// Create an editor with empty text.
	#[frb(sync)]
	#[allow(clippy::new_without_default)]
	pub fn new() -> Self {
		Self(canvist_ffi::EditorHandle::new())
	}

	/// Insert `text` before the Unicode scalar value at `offset`.
	///
	/// Offsets count Unicode scalar values (Dart `String.runes`), not UTF-16
	/// code units.
	#[frb(sync)]
	pub fn insert_text(&self, offset: u32, text: &str) -> Result<(), OffsetOutOfBounds> {
		self.0
			.insert_text(offset, text)
			.map_err(OffsetOutOfBounds::from)
	}

	/// The full text of the document.
	#[frb(sync)]
	pub fn text(&self) -> String {
		self.0.text()
	}

	/// The version of the Rust core.
	#[frb(sync)]
	pub fn version(&self) -> String {
		self.0.version()
	}
}

/// Thrown when an insertion offset is past the end of the text.
#[derive(Debug)]
pub struct OffsetOutOfBounds {
	/// The requested offset, in Unicode scalar values.
	pub offset: u32,
	/// The current text length, in Unicode scalar values.
	pub length: u32,
}

impl From<canvist_ffi::EditorError> for OffsetOutOfBounds {
	fn from(error: canvist_ffi::EditorError) -> Self {
		match error {
			canvist_ffi::EditorError::OffsetOutOfBounds { offset, length } => {
				Self { offset, length }
			}
		}
	}
}
