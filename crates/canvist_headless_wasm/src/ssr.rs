//! Server-side rendering, behind the `ssr` feature: `WasmRenderer`, which
//! `@canvist/headless/ssr` wraps.

use std::sync::Arc;

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::HeadlessError;
use canvist_headless::Renderer;
use canvist_headless::canvist_layout::StarterKitOptions;
use canvist_headless::canvist_layout::Theme;
use wasm_bindgen::prelude::*;

use crate::Result;
use crate::error;
use crate::format;
use crate::parse_json;

fn invalid(name: &str, reason: impl Into<String>) -> JsValue {
	error(HeadlessError::InvalidArgs {
		name: name.to_owned(),
		reason: reason.into(),
	})
}

/// Renders documents to HTML, CSS, and SVG laid out as the canvas editor
/// lays them out, with the StarterKit theme and the fonts registered here.
#[wasm_bindgen(js_name = WasmRenderer)]
pub struct WasmRenderer {
	engine: Engine,
	renderer: Renderer,
}

#[wasm_bindgen(js_class = WasmRenderer)]
impl WasmRenderer {
	/// A renderer for documents with the named extensions, themed by JSON
	/// `theme` (`@canvist/web`'s `ThemeOptions`).
	#[wasm_bindgen(constructor)]
	#[allow(
		clippy::needless_pass_by_value,
		reason = "wasm-bindgen hands JavaScript arrays over by value"
	)]
	pub fn new(extensions: Vec<String>, theme: &str) -> Result<WasmRenderer> {
		let engine = Engine::named(&extensions).map_err(error)?;
		let options = StarterKitOptions::from_json(&parse_json(theme)?)
			.map_err(|reason| invalid("theme", reason))?;
		let theme = Arc::new(Theme::starter_kit_with(&options));

		Ok(Self {
			engine,
			renderer: Renderer::with_theme(theme),
		})
	}

	/// Register a TrueType or OpenType font and return the family names it
	/// added.
	#[wasm_bindgen(js_name = registerFont)]
	pub fn register_font(&mut self, data: Vec<u8>) -> Vec<String> {
		self.renderer.register_font(data)
	}

	/// `input` read as `format`, rendered `width` pixels wide, as
	/// `[fragment, stylesheet]`.
	pub fn html(&self, format_name: &str, input: &str, width: f32) -> Result<Vec<String>> {
		let doc = self.parse(format_name, input)?;
		let html = self.renderer.html(&doc, positive_width(width)?);

		Ok(vec![html.html, html.css])
	}

	/// `input` read as `format`, as an SVG image `width` pixels wide.
	pub fn svg(&mut self, format_name: &str, input: &str, width: f32) -> Result<String> {
		let doc = self.parse(format_name, input)?;
		self.renderer
			.svg(&doc, positive_width(width)?)
			.map_err(error)
	}
}

impl WasmRenderer {
	fn parse(&self, format_name: &str, input: &str) -> Result<Doc> {
		self.engine
			.parse(format(format_name)?, input)
			.map_err(error)
	}
}

fn positive_width(width: f32) -> Result<f32> {
	if width.is_finite() && width > 0.0 {
		Ok(width)
	} else {
		Err(invalid(
			"width",
			"the width must be a positive number of pixels",
		))
	}
}
