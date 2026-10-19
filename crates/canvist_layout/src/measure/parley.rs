//! A measurer that shapes with `parley` (harfrust + skrifa), for desktop
//! hosts, server rendering, and snapshot tests with real fonts.

use std::borrow::Cow;

use parley::FontContext;
use parley::FontFamily;
use parley::FontStyle;
use parley::FontVariation;
use parley::FontVariations;
use parley::FontWeight;
use parley::Layout;
use parley::LayoutContext;
use parley::StyleProperty;
use parley::fontique::Blob;
use parley::fontique::Collection;
use parley::fontique::CollectionOptions;
use parley::fontique::FamilyId;
use parley::fontique::GenericFamily;
use parley::fontique::SourceCache;
use parley::setting::Tag;

use super::MeasureBatch;
use super::MeasureOutput;
use super::TextMeasurer;
use crate::FontMetrics;
use crate::FontSpec;

/// Generic families the first registered font stands in for, so CSS stacks
/// such as `Inter, ui-sans-serif, sans-serif` resolve without system fonts.
const GENERICS: [GenericFamily; 8] = [
	GenericFamily::SansSerif,
	GenericFamily::Serif,
	GenericFamily::Monospace,
	GenericFamily::SystemUi,
	GenericFamily::UiSansSerif,
	GenericFamily::UiSerif,
	GenericFamily::UiMonospace,
	GenericFamily::Emoji,
];

/// Measures text by shaping it with `parley`, using only fonts registered
/// with [`ParleyMeasurer::register_font`]. It never reads system fonts, so
/// results are identical on every machine with the same font files.
pub struct ParleyMeasurer {
	fonts: FontContext,
	layouts: LayoutContext<()>,
	fallback_set: bool,
}

impl std::fmt::Debug for ParleyMeasurer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ParleyMeasurer")
			.field("fallback_set", &self.fallback_set)
			.finish_non_exhaustive()
	}
}

impl Default for ParleyMeasurer {
	fn default() -> Self {
		Self::new()
	}
}

impl ParleyMeasurer {
	/// A measurer with no fonts.
	#[must_use]
	pub fn new() -> Self {
		let collection = Collection::new(CollectionOptions {
			shared: false,
			system_fonts: false,
		});

		Self {
			fonts: FontContext {
				collection,
				source_cache: SourceCache::default(),
			},
			layouts: LayoutContext::new(),
			fallback_set: false,
		}
	}

	/// Register every face in a TrueType or OpenType file (or collection) and
	/// return the family names it added. The first font registered also
	/// becomes the fallback for the generic families.
	pub fn register_font(&mut self, data: Vec<u8>) -> Vec<String> {
		let added = self.fonts.collection.register_fonts(Blob::from(data), None);
		let families: Vec<FamilyId> = added.iter().map(|(family, _)| *family).collect();

		if !self.fallback_set && !families.is_empty() {
			for generic in GENERICS {
				self.fonts
					.collection
					.set_generic_families(generic, families.iter().copied());
			}

			self.fallback_set = true;
		}

		families
			.into_iter()
			.filter_map(|id| self.fonts.collection.family_name(id).map(str::to_owned))
			.collect()
	}

	/// Shape `text` in `font`, returning its per-character advances and the
	/// metrics of the first run.
	fn shape(&mut self, font: &FontSpec, text: &str) -> (Vec<f32>, FontMetrics) {
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
		builder.push_default(StyleProperty::FontStyle(if font.italic {
			FontStyle::Italic
		} else {
			FontStyle::Normal
		}));
		// Browsers set a variable font's optical size to the font size
		// (`font-optical-sizing: auto`); fonts without the axis ignore it.
		let optical_size = [FontVariation::new(Tag::new(b"opsz"), font.size)];
		builder.push_default(StyleProperty::FontVariations(FontVariations::List(
			Cow::Borrowed(&optical_size),
		)));

		let mut layout = builder.build(text);
		layout.break_all_lines(None);

		char_advances(text, &layout)
	}
}

/// The per-character advances of shaped `text` and the metrics of its first
/// run. Each cluster's advance goes to the char it starts at, so the chars
/// after the first in a ligature or multi-char cluster advance nothing.
fn char_advances(text: &str, layout: &Layout<()>) -> (Vec<f32>, FontMetrics) {
	let starts: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
	let mut advances = vec![0.0; starts.len()];
	let char_index = |byte: usize| {
		starts
			.binary_search(&byte)
			.unwrap_or_else(|index| index.saturating_sub(1))
	};
	let mut metrics = None;

	for run in layout.lines().flat_map(|line| line.runs()) {
		let run_metrics = run.font_metrics();
		metrics.get_or_insert(FontMetrics {
			ascent: run_metrics.ascent,
			descent: run_metrics.descent,
		});

		for cluster in run.clusters() {
			let start = cluster.text_range().start;

			if let Some(advance) = advances.get_mut(char_index(start)) {
				*advance += cluster.advance();
			}
		}
	}

	(advances, metrics.unwrap_or_default())
}

impl TextMeasurer for ParleyMeasurer {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		for id in batch.metrics {
			let metrics = batch
				.font(*id)
				.map(|font| self.shape(font, "x").1)
				.unwrap_or_default();
			out.metrics.push(metrics);
		}

		for segment in batch.segments {
			let Some(font) = batch.font(segment.font) else {
				out.advances.extend(segment.text.chars().map(|_| 0.0));
				continue;
			};
			let (advances, _) = self.shape(font, segment.text);
			out.advances.extend(advances);
		}
	}
}
