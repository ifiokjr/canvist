//! Resolved fonts and the per-layout font table.

use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use rustc_hash::FxHashMap;

/// A fully resolved font: what a host needs to pick a face and shape text.
///
/// `family` is a CSS `font-family` list such as
/// `Inter, ui-sans-serif, sans-serif`, passed to the host unchanged, so the
/// canvas, the measurer, and server-rendered HTML all fall back through the
/// same faces.
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
	/// The CSS `font-family` list.
	pub family: Arc<str>,
	/// The font size in pixels.
	pub size: f32,
	/// The CSS weight, 100 to 900.
	pub weight: u16,
	/// Whether the face is italic.
	pub italic: bool,
}

impl FontSpec {
	/// The CSS `font` shorthand, e.g. `italic 700 16px Inter, sans-serif`,
	/// which Canvas2D's `font` property and CSS both accept.
	#[must_use]
	pub fn css(&self) -> String {
		self.to_string()
	}
}

impl fmt::Display for FontSpec {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let style = if self.italic { "italic" } else { "normal" };
		write!(f, "{style} {} {}px {}", self.weight, self.size, self.family)
	}
}

/// Fonts are compared by the bits of their size so they can key hash maps.
/// Sizes are never NaN because themes only produce finite sizes.
impl Eq for FontSpec {}

impl Hash for FontSpec {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.family.hash(state);
		self.size.to_bits().hash(state);
		self.weight.hash(state);
		self.italic.hash(state);
	}
}

/// An interned [`FontSpec`], valid within the [`FontBook`] that issued it.
///
/// Display lists carry the book's fonts alongside their items, so painters
/// can cache one native font object per id instead of re-parsing a font
/// string for every run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontId(pub(crate) u32);

impl FontId {
	/// The index of this font in [`FontBook::fonts`].
	#[must_use]
	pub const fn index(self) -> usize {
		self.0 as usize
	}
}

/// Vertical font metrics in pixels, as CSS uses them to build line boxes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FontMetrics {
	/// Distance from the baseline up to the ascender line. Positive.
	pub ascent: f32,
	/// Distance from the baseline down to the descender line. Positive.
	pub descent: f32,
}

/// Interns the fonts a layout uses.
#[derive(Clone, Debug, Default)]
pub struct FontBook {
	fonts: Vec<FontSpec>,
	ids: FxHashMap<FontSpec, FontId>,
}

impl FontBook {
	/// An empty book.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// The id for `spec`, adding it if it is new.
	pub fn intern(&mut self, spec: &FontSpec) -> FontId {
		if let Some(&id) = self.ids.get(spec) {
			return id;
		}

		let id = FontId(self.fonts.len() as u32);
		self.fonts.push(spec.clone());
		self.ids.insert(spec.clone(), id);
		id
	}

	/// The id of `spec`, if it has been interned.
	#[must_use]
	pub fn find(&self, spec: &FontSpec) -> Option<FontId> {
		self.ids.get(spec).copied()
	}

	/// The font with `id`, if this book issued it.
	#[must_use]
	pub fn get(&self, id: FontId) -> Option<&FontSpec> {
		self.fonts.get(id.index())
	}

	/// Every font, indexed by [`FontId::index`].
	#[must_use]
	pub fn fonts(&self) -> &[FontSpec] {
		&self.fonts
	}

	/// The number of fonts.
	#[must_use]
	pub fn len(&self) -> usize {
		self.fonts.len()
	}

	/// Whether the book is empty.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.fonts.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const BODY_SIZE: f32 = 16.0;
	const CODE_SIZE: f32 = 13.6;

	fn inter(size: f32) -> FontSpec {
		FontSpec {
			family: Arc::from("Inter, sans-serif"),
			size,
			weight: 400,
			italic: false,
		}
	}

	#[test]
	fn interning_is_stable() {
		let mut book = FontBook::new();
		let body = book.intern(&inter(BODY_SIZE));
		let code = book.intern(&inter(CODE_SIZE));

		assert_ne!(body, code);
		assert_eq!(book.intern(&inter(BODY_SIZE)), body);
		assert_eq!(book.get(code).map(|font| font.size), Some(CODE_SIZE));
		assert_eq!(book.len(), 2);
	}

	#[test]
	fn css_shorthand() {
		let font = FontSpec {
			weight: 700,
			italic: true,
			..inter(BODY_SIZE)
		};

		assert_eq!(font.css(), "italic 700 16px Inter, sans-serif");
	}
}
