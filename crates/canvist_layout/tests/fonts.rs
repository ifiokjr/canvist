//! Web font loading: forgetting one font's measurements relays out only the
//! blocks that use it.

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use canvist_layout::FixedMeasurer;
use canvist_layout::FontSpec;
use canvist_layout::MeasureBatch;
use canvist_layout::MeasureOutput;
use canvist_layout::TextMeasurer;
use serde_json::json;
use support::doc;
use support::layout;
use support::paragraph;

/// The family the StarterKit theme uses for code.
const MONO: &str = "JetBrains Mono";
/// How much wider the loaded face is than its fallback.
const LOADED_SCALE: f32 = 1.25;

/// A fixed measurer whose code font gets wider once it has "loaded", and
/// which records which fonts each batch asked about.
#[derive(Default)]
struct WebFonts {
	inner: FixedMeasurer,
	loaded: bool,
	segments_by_font: Vec<FontSpec>,
}

impl TextMeasurer for WebFonts {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		self.inner.measure(batch, out);
		let mut advance = 0;

		for segment in batch.segments {
			let font = batch.font(segment.font).expect("a known font");
			self.segments_by_font.push(font.clone());
			let count = segment.text.chars().count();

			if self.loaded && font.uses_family(MONO) {
				for value in &mut out.advances[advance..advance + count] {
					*value *= LOADED_SCALE;
				}
			}

			advance += count;
		}
	}
}

fn code_block(text: &str) -> serde_json::Value {
	json!({ "type": "codeBlock", "content": [{ "type": "text", "text": text }] })
}

#[test]
fn a_loaded_face_relays_out_only_its_blocks() {
	let doc = doc(json!([
		paragraph("Prose stays as it is."),
		code_block("let width = measure(text);"),
		paragraph("More prose, also untouched."),
	]));
	let code = doc.children(None)[1];
	let mut measurer = WebFonts::default();
	let mut layout = layout(&doc, 600.0);
	layout.layout_all(&doc, &mut measurer).expect("measured");
	let before = layout.block_rect(code).expect("a row");

	measurer.loaded = true;
	measurer.segments_by_font.clear();
	let invalidation = layout.forget_fonts(|font| font.uses_family(MONO));

	assert_eq!(invalidation.fonts, 1, "only the code font");
	assert_eq!(invalidation.blocks, 1, "only the code block");
	assert_eq!(invalidation.rects, vec![before]);

	let update = layout
		.ensure(&doc, &mut measurer, 0.0, 1000.0)
		.expect("measured");
	assert_eq!(update.laid_out, 1);
	assert_eq!(update.measurer_calls, 1);
	assert!(
		measurer
			.segments_by_font
			.iter()
			.all(|font| font.uses_family(MONO)),
		"prose segments stay cached: {:?}",
		measurer.segments_by_font
	);

	let stats = layout.measure_stats();
	assert!(stats.misses > 0);
	assert!(stats.bytes() > 0);
}

#[test]
fn unrelated_families_invalidate_nothing() {
	let doc = doc(json!([paragraph("Nothing here uses the face.")]));
	let mut measurer = WebFonts::default();
	let mut layout = layout(&doc, 600.0);
	layout.layout_all(&doc, &mut measurer).expect("measured");

	let invalidation = layout.forget_fonts(|font| font.uses_family("Not Used"));

	assert_eq!(invalidation.fonts, 0);
	assert_eq!(invalidation.blocks, 0);
	assert!(layout.is_fully_measured());
}

/// A fixed measurer that kerns every pair of touching kana by
/// [`KANA_KERN`], the way a shaper would inside one segment.
struct Kerned(FixedMeasurer);

const KANA_KERN: f32 = -0.5;

fn is_kana(c: char) -> bool {
	('\u{3040}'..='\u{30ff}').contains(&c)
}

impl TextMeasurer for Kerned {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		self.0.measure(batch, out);
		let mut advance = 0;

		for segment in batch.segments {
			let chars: Vec<char> = segment.text.chars().collect();

			for (index, pair) in chars.windows(2).enumerate() {
				if is_kana(pair[0]) && is_kana(pair[1]) {
					out.advances[advance + index] += KANA_KERN;
				}
			}

			advance += chars.len();
		}
	}
}

fn caret_xs(
	layout: &canvist_layout::DocumentLayout,
	block: canvist_model::BlockId,
	text: &str,
) -> Vec<f32> {
	text.char_indices()
		.map(|(offset, _)| offset as u32)
		.chain([text.len() as u32])
		.map(|offset| {
			layout
				.caret_rect(canvist_layout::Caret::new(canvist_model::Point::new(
					block, offset,
				)))
				.expect("a caret")
				.x
		})
		.collect()
}

#[test]
fn kerning_spans_items_that_touch() {
	// Every kana is its own item (Japanese may break between any two), but
	// the browser kerns them as one run.
	let text = "キスキス";
	let doc = doc(json!([paragraph(text)]));
	let block = doc.children(None)[0];
	let mut layout = layout(&doc, 600.0);
	layout
		.layout_all(&doc, &mut Kerned(FixedMeasurer::new()))
		.expect("measured");
	let mut kerned = Kerned(FixedMeasurer::new());
	layout.forget_measurements();
	layout.layout_all(&doc, &mut kerned).expect("measured");

	let xs = caret_xs(&layout, block, text);
	let steps: Vec<f32> = xs.windows(2).map(|pair| pair[1] - pair[0]).collect();
	let full = steps[3];

	assert!(
		steps[..3]
			.iter()
			.all(|step| (step - (full + KANA_KERN)).abs() < 1e-4),
		"{steps:?}"
	);
}

#[test]
fn incremental_relayout_keeps_kerning_at_the_window_edges() {
	use canvist_model::MarkSet;
	use canvist_model::Point;

	let before = "キスキスキス キスキス ";
	let mut doc = doc(json!([paragraph(&before.repeat(20))]));
	let block = doc.children(None)[0];
	let mut measurer = Kerned(FixedMeasurer::new());
	let mut incremental = layout(&doc, 300.0);
	incremental.forget_measurements();
	incremental
		.layout_all(&doc, &mut measurer)
		.expect("measured");

	let mut transaction = doc.transaction();
	transaction
		.insert_text(Point::new(block, 6), "キ", MarkSet::empty())
		.expect("valid point");
	let commit = transaction.commit();
	incremental.apply(&doc, &commit);
	incremental
		.layout_all(&doc, &mut measurer)
		.expect("measured");

	let mut fresh = layout(&doc, 300.0);
	fresh.forget_measurements();
	fresh
		.layout_all(&doc, &mut Kerned(FixedMeasurer::new()))
		.expect("measured");
	let text = doc.inline(block).expect("a paragraph").as_str().to_owned();

	assert_eq!(
		caret_xs(&incremental, block, &text),
		caret_xs(&fresh, block, &text)
	);
}

/// A fixed measurer that kerns a space before `T` by [`SPACE_KERN`], as
/// Times-compatible fonts such as Liberation Serif do.
struct SpaceKerned(FixedMeasurer);

const SPACE_KERN: f32 = -1.5;

impl TextMeasurer for SpaceKerned {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		self.0.measure(batch, out);
		let mut advance = 0;

		for segment in batch.segments {
			let chars: Vec<char> = segment.text.chars().collect();

			for (index, pair) in chars.windows(2).enumerate() {
				if pair == [' ', 'T'] {
					out.advances[advance + index] += SPACE_KERN;
				}
			}

			advance += chars.len();
		}
	}
}

#[test]
fn kerning_spans_the_space_between_words() {
	// Items end after their trailing space, so the space and the `T` after
	// it are measured apart, but the browser shapes them together.
	let text = "To Ty To";
	let doc = doc(json!([paragraph(text)]));
	let block = doc.children(None)[0];
	let mut kerned = layout(&doc, 600.0);
	kerned.forget_measurements();
	kerned
		.layout_all(&doc, &mut SpaceKerned(FixedMeasurer::new()))
		.expect("measured");
	let mut plain = layout(&doc, 600.0);
	plain.forget_measurements();
	plain
		.layout_all(&doc, &mut FixedMeasurer::new())
		.expect("measured");

	let kerned = caret_xs(&kerned, block, text);
	let plain = caret_xs(&plain, block, text);
	let shift: Vec<f32> = kerned.iter().zip(&plain).map(|(a, b)| a - b).collect();

	// Each " T" pair pulls everything after it left by SPACE_KERN.
	let expected = [
		0.0,
		0.0,
		0.0,
		SPACE_KERN,
		SPACE_KERN,
		SPACE_KERN,
		2.0 * SPACE_KERN,
		2.0 * SPACE_KERN,
		2.0 * SPACE_KERN,
	];
	assert!(
		shift
			.iter()
			.zip(expected)
			.all(|(a, b)| (a - b).abs() < 1e-4),
		"{shift:?}"
	);
}
