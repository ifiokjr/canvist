//! Fonts: the machine's own, the bundled Atkinson Hyperlegible, and any the
//! app loads from bytes. Layout measures and painting shapes with the same
//! set, so glyphs land exactly where layout put them.

use std::sync::Arc;

use canvist_layout::ParleyMeasurer;
use canvist_paint::BUNDLED_FONT;
use canvist_paint::Shaper;

/// Which fonts the editor can use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontSource {
	/// The fonts installed on this machine, plus the bundled font and any
	/// the app registers. Generic families such as `system-ui` resolve to the
	/// platform's own. This is what apps want.
	#[default]
	System,
	/// Only the bundled font and the fonts the app registers, which stand in
	/// for every generic family. Output is identical on every machine, so
	/// tests and screenshots use it.
	Bundled,
}

/// The fonts an editor measures and paints with.
#[derive(Clone, Debug)]
pub struct Fonts {
	source: FontSource,
	/// Fonts registered from bytes, in order, so every new shaper gets them.
	registered: Vec<Arc<[u8]>>,
	/// Bumped whenever a font is registered, so painters know to rebuild
	/// their shapers.
	generation: u64,
}

impl Fonts {
	pub(crate) fn new(source: FontSource) -> Self {
		Self {
			source,
			registered: Vec::new(),
			generation: 0,
		}
	}

	/// Where the fonts come from.
	#[must_use]
	pub fn source(&self) -> FontSource {
		self.source
	}

	/// Changes whenever a font is registered. A host that keeps a painter
	/// across frames rebuilds its shaper with [`Fonts::shaper`] when this
	/// differs from the last value it saw.
	#[must_use]
	pub fn generation(&self) -> u64 {
		self.generation
	}

	/// A shaper for a painter, with exactly the fonts layout measures with.
	#[must_use]
	pub fn shaper(&self) -> Shaper {
		let mut shaper = match self.source {
			FontSource::System => Shaper::with_system_fonts(),
			FontSource::Bundled => Shaper::new(),
		};

		for data in &self.registered {
			shaper.register_font(data.to_vec());
		}

		shaper
	}

	/// The measurer layout uses, with the same fonts.
	pub(crate) fn measurer(&self) -> ParleyMeasurer {
		let mut measurer = match self.source {
			FontSource::System => ParleyMeasurer::with_system_fonts(),
			FontSource::Bundled => ParleyMeasurer::new(),
		};

		measurer.register_font(BUNDLED_FONT.to_vec());

		for data in &self.registered {
			measurer.register_font(data.to_vec());
		}

		measurer
	}

	/// Remember `data` for future shapers and bump the generation.
	pub(crate) fn add(&mut self, data: Arc<[u8]>) {
		self.registered.push(data);
		self.generation += 1;
	}
}
