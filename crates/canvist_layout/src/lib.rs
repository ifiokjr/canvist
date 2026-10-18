//! # `canvist_layout`
//!
//! Incremental text and block layout for the canvist editor.
//!
//! - A [`Theme`] maps block types, marks, and embeds to styles. Rules are
//!   data, keyed by schema names, so extensions contribute their own.
//! - A [`TextMeasurer`] measures text for the host platform. Layout batches
//!   every missing measurement of a relayout into one call and caches results
//!   by (font, segment). [`FixedMeasurer`] is a deterministic measurer for
//!   tests and headless editing.
//! - Text layout breaks lines at UAX #14 opportunities with CSS `pre-wrap`
//!   rules, orders bidi text with UAX #9, and relays out only the lines
//!   around an edit.

mod font;
mod geometry;
#[expect(
	dead_code,
	reason = "document layout, the next layer of this stack, is the caller"
)]
mod measure;
#[expect(
	dead_code,
	reason = "document layout, the next layer of this stack, is the caller"
)]
mod style;
#[expect(
	dead_code,
	unused_imports,
	reason = "document layout, the next layer of this stack, is the caller"
)]
mod text;
mod theme;

use canvist_model::BlockId;
use canvist_model::Point;
pub use font::FontBook;
pub use font::FontId;
pub use font::FontMetrics;
pub use font::FontSpec;
pub use geometry::Color;
pub use geometry::Insets;
pub use geometry::Rect;
pub use measure::FixedMeasurer;
pub use measure::MeasureBatch;
pub use measure::MeasureOutput;
pub use measure::SegmentRequest;
pub use measure::TextMeasurer;
pub use style::BlockStyle;
pub use theme::AttrBinding;
pub use theme::BlockPatch;
pub use theme::BlockRule;
pub use theme::Border;
pub use theme::Chrome;
pub use theme::Decorations;
pub use theme::EmbedRule;
pub use theme::EmbedStyle;
pub use theme::FontSize;
pub use theme::LeafStyle;
pub use theme::MarkRule;
pub use theme::MarkerKind;
pub use theme::MarkerStyle;
pub use theme::TextPatch;
pub use theme::TextStyle;
pub use theme::Theme;
pub use theme::ThemeBuilder;

/// Overrides UAX #14 line break opportunities, to match a browser's
/// tailoring (Chromium, for example, suppresses some breaks around ASCII
/// punctuation). Called with the character before the previous one, the
/// previous character, and the next character; return `Some(true)` to allow a
/// break between them, `Some(false)` to forbid it, or `None` to keep the
/// default. Never called across forced breaks.
pub type BreakOverride = fn(Option<char>, char, char) -> Option<bool>;

/// Which line a caret belongs to when its offset is ambiguous.
///
/// At a soft wrap, the offset that ends one line also starts the next. At a
/// bidi boundary, one offset has two visual positions. Upstream sticks to the
/// character before the offset, downstream to the character after it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
	/// Attach to the character before the offset.
	Upstream,
	/// Attach to the character after the offset.
	#[default]
	Downstream,
}

/// A caret: a model point plus the affinity that disambiguates where it is
/// drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Caret {
	/// The model position.
	pub point: Point,
	/// Which side the caret attaches to.
	pub affinity: Affinity,
}

impl Caret {
	/// A downstream caret at `point`.
	#[must_use]
	pub const fn new(point: Point) -> Self {
		Self {
			point,
			affinity: Affinity::Downstream,
		}
	}

	/// A caret with an explicit affinity.
	#[must_use]
	pub const fn with_affinity(point: Point, affinity: Affinity) -> Self {
		Self { point, affinity }
	}
}

/// Errors raised by layout.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LayoutError {
	/// A [`TextMeasurer`] returned the wrong number of results.
	#[error(
		"the measurer returned {} font metrics and {} advances, but {} and {} were requested",
		returned[0], returned[1], requested[0], requested[1]
	)]
	MeasurerContract {
		/// The requested number of font metrics and character advances.
		requested: [u32; 2],
		/// The returned number of font metrics and character advances.
		returned: [u32; 2],
	},
	/// A measurement was missing after the cache was flushed.
	#[error("a text measurement was missing after measuring")]
	MissingMeasurement,
	/// The layout refers to a block the document no longer has, or whose
	/// content kind changed: a commit was not passed to
	/// `DocumentLayout::apply`.
	#[error("block {0} is out of sync with the document; apply every commit to the layout")]
	OutOfSync(BlockId),
}
