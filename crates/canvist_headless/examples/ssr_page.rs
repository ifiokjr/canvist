//! Server-render the page that `examples/web/ssr.html` hydrates: the
//! document's HTML and CSS, laid out with the same Inter file the browser
//! loads, so the live editor that replaces it moves nothing.
//!
//! Reads a page template with `<!-- canvist:css -->` and
//! `<!-- canvist:html -->` placeholders and writes the rendered page.
//!
//! ```sh
//! cargo run -p canvist_headless --features ssr,starter-kit --example ssr_page -- \
//! 	examples/web/ssr.template.html examples/web/public/fonts/inter-variable.ttf examples/web/ssr.html
//! ```

use std::sync::Arc;

use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Renderer;
use canvist_layout::StarterKitOptions;
use canvist_layout::Theme;

/// The width the page gives the editor, in CSS px.
const WIDTH: f32 = 640.0;

/// The document, as Markdown.
const CONTENT: &str = "# Rendered on the server

This page arrived as **HTML**. Once Inter has loaded, the live editor takes its place without \
                       moving a pixel: it measures text with the same font file, so every line \
                       wraps at the *same character*.

- Click anywhere and start typing.
- Press `Mod-b` for **bold**.

> Server rendering gives readers the text before any JavaScript runs.
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut args = std::env::args().skip(1);
	let (Some(template), Some(font), Some(output)) = (args.next(), args.next(), args.next()) else {
		return Err("usage: ssr_page <template.html> <font.ttf> <output.html>".into());
	};

	// The same options the browser editor's `theme` sets.
	let theme = Theme::starter_kit_with(&StarterKitOptions {
		body_family: Arc::from("Inter, sans-serif"),
		..StarterKitOptions::default()
	});
	let mut renderer = Renderer::with_theme(Arc::new(theme));
	renderer.register_font(std::fs::read(font)?);

	let doc = Engine::starter_kit().parse(Format::Markdown, CONTENT)?;
	let document = renderer.html(&doc, WIDTH);
	let page = std::fs::read_to_string(template)?
		.replace("<!-- canvist:css -->", &document.css)
		.replace("<!-- canvist:html -->", &document.html);
	std::fs::write(output, page)?;
	Ok(())
}
