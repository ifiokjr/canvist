//! Shaping text runs into glyphs for the GPU and CPU backends, with the
//! same `parley` shaping (and the same bundled font) the canvas measures
//! with, so glyphs land exactly where layout put them.

use std::borrow::Cow;
use std::sync::Arc;

use canvist_layout::FontSpec;
use parley::FontContext;
use parley::FontFamily;
use parley::FontStyle;
use parley::FontWeight;
use parley::LayoutContext;
use parley::PositionedLayoutItem;
use parley::StyleProperty;
use parley::fontique::Blob;
use parley::fontique::Collection;
use parley::fontique::CollectionOptions;
use parley::fontique::GenericFamily;
use parley::fontique::SourceCache;
use peniko::FontData;
use rustc_hash::FxHashMap;

/// The font every canvas renderer and measurer ships with: Atkinson
/// Hyperlegible (SIL Open Font License, see `fonts/`).
pub const BUNDLED_FONT: &[u8] = include_bytes!("../fonts/AtkinsonHyperlegible-Regular.ttf");

/// Generic families the bundled font stands in for.
const GENERICS: [GenericFamily; 7] = [
	GenericFamily::SansSerif,
	GenericFamily::Serif,
	GenericFamily::Monospace,
	GenericFamily::SystemUi,
	GenericFamily::UiSansSerif,
	GenericFamily::UiSerif,
	GenericFamily::UiMonospace,
];
/// Shaped runs kept before the cache starts over.
const CACHE_LIMIT: usize = 4_096;

/// One positioned glyph, relative to the run's origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
	/// The glyph id in the font.
	pub id: u32,
	/// The horizontal position.
	pub x: f32,
	/// The vertical position.
	pub y: f32,
}

/// A run of glyphs in one font.
#[derive(Clone, Debug)]
pub struct ShapedRun {
	/// The font.
	pub font: FontData,
	/// The font size in pixels.
	pub size: f32,
	/// The glyphs.
	pub glyphs: Vec<Glyph>,
	/// The variable font instance to draw, as normalized design
	/// coordinates (such as a bold weight). Empty for static fonts.
	pub coords: Vec<i16>,
	/// Whether to embolden the outlines: the style asked for a heavier
	/// weight than the font has.
	pub embolden: bool,
	/// A slant in degrees for a font without the italic the style asked
	/// for.
	pub skew: Option<f32>,
}

impl ShapedRun {
	/// How far synthetic bold grows outlines on each side, in pixels: about
	/// a 48th of the font size, as browsers embolden.
	#[must_use]
	pub fn embolden_amount(&self) -> f64 {
		if self.embolden {
			f64::from(self.size) / 48.0
		} else {
			0.0
		}
	}

	/// The horizontal shear for synthetic italic (the tangent of the slant),
	/// if any. Backends apply it in their own glyph space: vello's is y up,
	/// `vello_cpu`'s y down, so the signs differ.
	#[must_use]
	pub fn shear(&self) -> Option<f64> {
		self.skew
			.map(|degrees| f64::from(degrees).to_radians().tan())
	}
}

/// What a shaped run is cached by: family, size and letter spacing bits,
/// weight, italic, and text.
type ShapeKey = (String, u32, u16, bool, String);

/// Shapes text with registered fonts, caching results by font and text.
pub struct Shaper {
	fonts: FontContext,
	layouts: LayoutContext<()>,
	cache: FxHashMap<ShapeKey, Arc<[ShapedRun]>>,
}

impl std::fmt::Debug for Shaper {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Shaper")
			.field("cached", &self.cache.len())
			.finish_non_exhaustive()
	}
}

impl Default for Shaper {
	fn default() -> Self {
		Self::new()
	}
}

impl Shaper {
	/// A shaper with the bundled font, which also stands in for the generic
	/// families.
	#[must_use]
	pub fn new() -> Self {
		let collection = Collection::new(CollectionOptions {
			shared: false,
			system_fonts: false,
		});
		let mut shaper = Self {
			fonts: FontContext {
				collection,
				source_cache: SourceCache::default(),
			},
			layouts: LayoutContext::new(),
			cache: FxHashMap::default(),
		};
		let families = shaper.register(BUNDLED_FONT.to_vec());

		for generic in GENERICS {
			shaper
				.fonts
				.collection
				.set_generic_families(generic, families.iter().copied());
		}

		shaper
	}

	/// A shaper that also reads the fonts installed on this machine, for
	/// desktop hosts. Generic families such as `sans-serif` and `system-ui`
	/// resolve to the platform's own, and the bundled font is available by
	/// name. Pair it with `ParleyMeasurer::with_system_fonts`, so layout
	/// measures with the faces painting draws.
	#[must_use]
	pub fn with_system_fonts() -> Self {
		let collection = Collection::new(CollectionOptions {
			shared: false,
			system_fonts: true,
		});
		let mut shaper = Self {
			fonts: FontContext {
				collection,
				source_cache: SourceCache::default(),
			},
			layouts: LayoutContext::new(),
			cache: FxHashMap::default(),
		};
		shaper.register(BUNDLED_FONT.to_vec());
		shaper
	}

	/// Register a TrueType or OpenType font for themes that name it.
	pub fn register_font(&mut self, data: Vec<u8>) {
		self.register(data);
		self.cache.clear();
	}

	fn register(&mut self, data: Vec<u8>) -> Vec<parley::fontique::FamilyId> {
		self.fonts
			.collection
			.register_fonts(Blob::from(data), None)
			.into_iter()
			.map(|(family, _)| family)
			.collect()
	}

	/// The glyph runs of `text` in `font`, positioned from the run's origin
	/// on its baseline. Usually one run; font fallback can split it.
	pub fn shape(&mut self, font: &FontSpec, text: &str, letter_spacing: f32) -> Arc<[ShapedRun]> {
		let key = (
			font.family.to_string(),
			font.size.to_bits() ^ letter_spacing.to_bits().rotate_left(16),
			font.weight,
			font.italic,
			text.to_owned(),
		);

		if let Some(found) = self.cache.get(&key) {
			return found.clone();
		}

		if self.cache.len() >= CACHE_LIMIT {
			self.cache.clear();
		}

		let runs: Arc<[ShapedRun]> = self.shape_uncached(font, text, letter_spacing).into();
		self.cache.insert(key, runs.clone());
		runs
	}

	fn shape_uncached(
		&mut self,
		font: &FontSpec,
		text: &str,
		letter_spacing: f32,
	) -> Vec<ShapedRun> {
		let mut builder = self
			.layouts
			.ranged_builder(&mut self.fonts, text, 1.0, false);
		builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
			Cow::Borrowed(&font.family),
		)));
		builder.push_default(StyleProperty::FontSize(font.size));
		builder.push_default(StyleProperty::FontWeight(FontWeight::new(f32::from(
			font.weight,
		))));
		builder.push_default(StyleProperty::LetterSpacing(letter_spacing));
		builder.push_default(StyleProperty::FontStyle(if font.italic {
			FontStyle::Italic
		} else {
			FontStyle::Normal
		}));

		let mut layout = builder.build(text);
		layout.break_all_lines(None);

		layout
			.lines()
			.flat_map(|line| line.items().filter_map(glyph_run).collect::<Vec<_>>())
			.collect()
	}
}

/// The shaped glyphs of a layout item, if it is a glyph run.
fn glyph_run(item: PositionedLayoutItem<'_, ()>) -> Option<ShapedRun> {
	match item {
		PositionedLayoutItem::GlyphRun(run) => Some(shaped(&run)),
		PositionedLayoutItem::InlineBox(_) => None,
	}
}

/// A parley glyph run, re-based so `y` is relative to the baseline.
fn shaped(run: &parley::GlyphRun<'_, ()>) -> ShapedRun {
	let baseline = run.baseline();
	let glyphs = run
		.positioned_glyphs()
		.map(|glyph| {
			Glyph {
				id: glyph.id,
				x: glyph.x,
				y: glyph.y - baseline,
			}
		})
		.collect();

	let synthesis = run.run().synthesis();

	ShapedRun {
		font: run.run().font().clone(),
		size: run.run().font_size(),
		glyphs,
		coords: run
			.run()
			.normalized_coords()
			.iter()
			.map(|coord| coord.to_bits())
			.collect(),
		embolden: synthesis.embolden(),
		skew: synthesis.skew(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn shapes_with_the_bundled_font_and_caches() {
		let mut shaper = Shaper::new();
		let font = FontSpec {
			family: "Atkinson Hyperlegible, sans-serif".into(),
			size: 16.0,
			weight: 400,
			italic: false,
		};
		let runs = shaper.shape(&font, "Hello", 0.0);
		let glyphs: usize = runs.iter().map(|run| run.glyphs.len()).sum();

		assert_eq!(glyphs, 5);
		assert!(runs[0].glyphs.windows(2).all(|pair| pair[0].x < pair[1].x));
		assert!(Arc::ptr_eq(&runs, &shaper.shape(&font, "Hello", 0.0)));
	}
}
