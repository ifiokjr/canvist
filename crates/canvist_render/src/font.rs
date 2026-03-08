//! Font loading and glyph caching.
//!
//! Provides a [`FontCache`] that loads fonts via [`fontdue`] and caches
//! rasterised glyphs for efficient text rendering.

use std::collections::HashMap;

/// A cache of loaded fonts and their rasterised glyphs.
///
/// The font cache is shared across render frames to avoid re-rasterising
/// glyphs that have already been drawn. In a production editor this would
/// include LRU eviction, but for now it grows unbounded.
///
/// # Examples
///
/// ```
/// use canvist_render::FontCache;
///
/// let mut cache = FontCache::new();
///
/// // Load a font from bytes.
/// let font_bytes = include_bytes!("../../../docs/src/readme.md"); // placeholder
/// // cache.load_font("MyFont", font_bytes);
/// ```
pub struct FontCache {
	/// Loaded fonts keyed by family name.
	fonts: HashMap<String, fontdue::Font>,
}

impl FontCache {
	/// Create a new empty font cache.
	#[must_use]
	pub fn new() -> Self {
		Self {
			fonts: HashMap::new(),
		}
	}

	/// Load a font from raw bytes and register it under the given family name.
	///
	/// # Errors
	///
	/// Returns an error string if the font data is invalid.
	pub fn load_font(&mut self, family: impl Into<String>, data: &[u8]) -> Result<(), String> {
		let settings = fontdue::FontSettings::default();
		let font = fontdue::Font::from_bytes(data, settings)?;
		self.fonts.insert(family.into(), font);
		Ok(())
	}

	/// Look up a loaded font by family name.
	#[must_use]
	pub fn get(&self, family: &str) -> Option<&fontdue::Font> {
		self.fonts.get(family)
	}

	/// Return the number of loaded fonts.
	#[must_use]
	pub fn font_count(&self) -> usize {
		self.fonts.len()
	}

	/// Whether any fonts are loaded.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.fonts.is_empty()
	}
}

impl Default for FontCache {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn empty_cache() {
		let cache = FontCache::new();
		assert!(cache.is_empty());
		assert_eq!(cache.font_count(), 0);
		assert!(cache.get("Arial").is_none());
	}
}
