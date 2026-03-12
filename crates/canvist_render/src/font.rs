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

/// Text measurer that uses [`fontdue`] for accurate glyph metrics.
///
/// This enables precise text measurement on native (non-WASM) platforms
/// without needing a `Canvas2D` context. Pass a reference to a [`FontCache`]
/// with loaded fonts.
///
/// When a requested font family isn't loaded, falls back to the heuristic
/// `font_size × 0.6` per character.
pub struct FontdueMeasure<'a> {
	cache: &'a FontCache,
}

impl<'a> FontdueMeasure<'a> {
	/// Create a measurer backed by the given font cache.
	#[must_use]
	pub fn new(cache: &'a FontCache) -> Self {
		Self { cache }
	}
}

impl canvist_core::layout::TextMeasure for FontdueMeasure<'_> {
	fn measure_char(&self, ch: char, style: &canvist_core::Style) -> f32 {
		let resolved = style.resolve();
		if let Some(font) = self.cache.get(&resolved.font_family) {
			let (metrics, _) = font.rasterize(ch, resolved.font_size);
			metrics.advance_width + resolved.letter_spacing
		} else {
			// Fallback heuristic.
			resolved.font_size * 0.6 + resolved.letter_spacing
		}
	}

	fn measure_text(&self, text: &str, style: &canvist_core::Style) -> f32 {
		text.chars().map(|ch| self.measure_char(ch, style)).sum()
	}
}

#[cfg(test)]
mod tests {
	use canvist_core::Style;
	use canvist_core::layout::TextMeasure;

	use super::*;

	#[test]
	fn empty_cache() {
		let cache = FontCache::new();
		assert!(cache.is_empty());
		assert_eq!(cache.font_count(), 0);
		assert!(cache.get("Arial").is_none());
	}

	#[test]
	fn fontdue_measure_fallback_heuristic() {
		// When no font is loaded, FontdueMeasure falls back to 0.6 × font_size.
		let cache = FontCache::new();
		let measurer = FontdueMeasure::new(&cache);
		let style = Style::new().font_size(20.0);
		let width = measurer.measure_char('A', &style);
		assert!((width - 12.0).abs() < 0.01, "expected 12.0, got {width}");
	}

	#[test]
	fn fontdue_measure_text() {
		let cache = FontCache::new();
		let measurer = FontdueMeasure::new(&cache);
		let style = Style::new().font_size(16.0);
		let width = measurer.measure_text("Hello", &style);
		// 5 chars × 16 × 0.6 = 48.0
		assert!((width - 48.0).abs() < 0.01, "expected 48.0, got {width}");
	}

	#[test]
	fn fontdue_measure_with_letter_spacing() {
		let cache = FontCache::new();
		let measurer = FontdueMeasure::new(&cache);
		let style = Style::new().font_size(16.0).letter_spacing(2.0);
		let width = measurer.measure_char('A', &style);
		// 16 × 0.6 + 2.0 = 11.6
		assert!((width - 11.6).abs() < 0.01, "expected 11.6, got {width}");
	}
}
