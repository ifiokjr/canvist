//! Server-side rendering through `canvist_layout`.

use std::sync::Arc;

use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::HtmlDocument;
use canvist_layout::HtmlOptions;
use canvist_layout::LayoutOptions;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Theme;

use crate::Doc;
use crate::HeadlessError;

/// Atkinson Hyperlegible, under the SIL Open Font License (see
/// `fonts/AtkinsonHyperlegible-OFL.txt`).
const BUNDLED_FONT: &[u8] = include_bytes!("../fonts/AtkinsonHyperlegible-Regular.ttf");

/// Renders documents to HTML and SVG the way the canvas editor lays them
/// out, so a page can show a document before the editor loads and swap in
/// the canvas without the text moving.
///
/// SVG needs real text measurements, so the renderer shapes text with
/// `parley` and its own fonts, never the system's: the bundled Atkinson
/// Hyperlegible stands in for every generic family until
/// [`Renderer::register_font`] adds the app's fonts. Measurements are cached
/// per renderer, so keep one per thread and reuse it.
///
/// ```
/// use canvist_headless::Engine;
/// use canvist_headless::Format;
/// use canvist_headless::Renderer;
///
/// let doc = Engine::starter_kit().parse(Format::Markdown, "# Hello")?;
/// let mut renderer = Renderer::new();
///
/// assert!(renderer.html(&doc, 600.0).html.contains("Hello"));
/// assert!(renderer.svg(&doc, 600.0)?.starts_with("<svg"));
/// # Ok::<(), canvist_headless::HeadlessError>(())
/// ```
#[derive(Debug)]
pub struct Renderer {
	theme: Arc<Theme>,
	measurer: ParleyMeasurer,
}

impl Default for Renderer {
	fn default() -> Self {
		Self::new()
	}
}

impl Renderer {
	/// A renderer with the StarterKit theme and the bundled font.
	#[must_use]
	pub fn new() -> Self {
		Self::with_theme(Arc::new(Theme::starter_kit()))
	}

	/// A renderer with `theme` and the bundled font.
	#[must_use]
	pub fn with_theme(theme: Arc<Theme>) -> Self {
		let mut measurer = ParleyMeasurer::new();
		measurer.register_font(BUNDLED_FONT.to_vec());
		Self { theme, measurer }
	}

	/// Register a TrueType or OpenType font and return the family names it
	/// added, for themes that name them.
	pub fn register_font(&mut self, data: Vec<u8>) -> Vec<String> {
		self.measurer.register_font(data)
	}

	/// Semantic HTML and its stylesheet, `width` pixels wide. The CSS pins
	/// line breaking so the HTML wraps where the canvas does.
	#[must_use]
	pub fn html(&self, doc: &Doc, width: f32) -> HtmlDocument {
		canvist_layout::to_html(doc.document(), &self.theme, &HtmlOptions::new(width))
	}

	/// The whole document as an SVG image `width` pixels wide.
	pub fn svg(&mut self, doc: &Doc, width: f32) -> Result<String, HeadlessError> {
		Ok(canvist_layout::to_svg(&self.display_list(doc, width)?))
	}

	/// The whole document laid out `width` pixels wide, as the display list
	/// [`Renderer::svg`] paints. Native hosts paint it with their own
	/// painter to check that their layout matches server rendering.
	pub fn display_list(&mut self, doc: &Doc, width: f32) -> Result<DisplayList, HeadlessError> {
		let document = doc.document();
		let mut layout =
			DocumentLayout::new(document, self.theme.clone(), LayoutOptions::new(width));
		layout.layout_all(document, &mut self.measurer)?;

		Ok(layout.display_list(&DisplayOptions::viewport(0.0, layout.height())))
	}
}
