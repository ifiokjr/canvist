#![doc(
	html_logo_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_layout`
//!
//! Incremental text and block layout for the canvist editor, and the display
//! list every painter consumes.
//!
//! - A [`Theme`] maps block types, marks, and embeds to styles. Rules are
//!   data, keyed by schema names, so extensions contribute their own.
//! - A [`TextMeasurer`] measures text for the host platform. Layout batches
//!   every missing measurement of a relayout into one call and caches results
//!   by (font, segment).
//! - [`DocumentLayout`] breaks lines (UAX #14, CSS `pre-wrap` rules), orders
//!   bidi text (UAX #9), stacks blocks with their containers, estimates the
//!   height of blocks outside the viewport, and relays out only the blocks a
//!   [`Commit`](canvist_model::Commit) touched.
//! - Geometry queries: [`DocumentLayout::hit_test`],
//!   [`DocumentLayout::caret_rect`], [`DocumentLayout::selection_rects`], and
//!   the [`LayoutNavigator`] the editor uses for vertical caret motion.
//! - [`DisplayList`]: positioned text runs, rectangles, decorations, and
//!   node-view slots, with [`DisplayList::damage`] for dirty rectangles.
//! - Server rendering: [`to_svg`] paints a display list, and [`to_html`]
//!   writes semantic HTML plus CSS that wraps lines exactly where the canvas
//!   does. See the [CSS contract](https://github.com/ifiokjr/canvist/blob/main/docs/src/v2/ssr-css-contract.md).
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_layout::DisplayOptions;
//! use canvist_layout::DocumentLayout;
//! use canvist_layout::FixedMeasurer;
//! use canvist_layout::LayoutOptions;
//! use canvist_layout::Theme;
//! use canvist_model::BlockSpec;
//! use canvist_model::Document;
//! use canvist_model::MarkSet;
//! use canvist_model::Point;
//! use canvist_model::Schema;
//!
//! let schema = Arc::new(Schema::builder().block(BlockSpec::text("paragraph")).build()?);
//! let mut doc = Document::new(schema, 1);
//! let block = doc.first_text_block().expect("new documents have one block");
//! let mut transaction = doc.transaction();
//! transaction.insert_text(Point::new(block, 0), "Hello, layout!", MarkSet::empty())?;
//! transaction.commit();
//!
//! let mut measurer = FixedMeasurer::new();
//! let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), LayoutOptions::new(80.0));
//! layout.layout_all(&doc, &mut measurer)?;
//!
//! // 14 characters at 9.6 px do not fit in 80 px, so the paragraph wraps.
//! assert_eq!(layout.line_count(block), Some(2));
//!
//! let display = layout.display_list(&DisplayOptions::viewport(0.0, 600.0));
//! assert!(canvist_layout::to_svg(&display).contains("Hello, "));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod font;
mod geometry;
mod measure;
mod style;
mod text;
mod theme;

mod display;
mod document;
#[cfg(feature = "editor")]
mod editor;
mod navigate;
mod render;

use canvist_model::BlockId;
use canvist_model::Point;
pub use display::DecorationKind;
pub use display::DisplayItem;
pub use display::DisplayList;
pub use display::DisplayOptions;
pub use display::RectKind;
pub use display::SlotKind;
pub use display::TextRun;
pub use document::Damage;
pub use document::DocumentLayout;
pub use document::LayoutOptions;
pub use document::ViewportUpdate;
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
#[cfg(feature = "parley")]
pub use measure::ParleyMeasurer;
pub use measure::SegmentRequest;
pub use measure::TextMeasurer;
pub use navigate::LayoutNavigator;
pub use navigate::LineEdge;
pub use navigate::VerticalDirection;
pub use navigate::VerticalMotion;
pub use render::HtmlDocument;
pub use render::HtmlOptions;
pub use render::to_html;
pub use render::to_svg;
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
	/// [`DocumentLayout::apply`].
	#[error("block {0} is out of sync with the document; apply every commit to the layout")]
	OutOfSync(BlockId),
}
