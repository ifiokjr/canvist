# Quick start: Server-side rendering

In this guide you render a canvist document to HTML and SVG on a server, laid out exactly as the editor paints it. Use it for fast first paint, SEO, readers without JavaScript, email, previews, and thumbnails.

**Status: available in Rust, coming soon for JavaScript.** Rendering ships today in `canvist_headless` behind the `ssr` feature. `@canvist/ssr`, for Next.js, Nuxt, and other JavaScript servers, is coming soon.

## 1. Install

```sh
cargo add canvist_headless --git https://github.com/ifiokjr/canvist --features ssr
```

## 2. Render HTML and SVG

```rust
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Renderer;

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let doc = Engine::starter_kit().parse(Format::Markdown, "# Release notes\n\n- Faster **typing**")?;

	// Keep one renderer per thread: it caches text measurements.
	let mut renderer = Renderer::new();

	// Semantic HTML plus a stylesheet, laid out 640 pixels wide.
	let html = renderer.html(&doc, 640.0);
	assert!(html.html.starts_with(r#"<div class="cv-doc""#));
	assert!(html.css.contains(".cv-doc"));

	// A complete page, ready to serve.
	let page = html.page();
	assert!(page.contains("Release notes"));

	// An SVG image of the whole document, for previews and thumbnails.
	let svg = renderer.svg(&doc, 640.0)?;
	assert!(svg.starts_with("<svg"));
	Ok(())
}
```

## How the HTML matches the editor

The stylesheet pins line breaking, fonts, and spacing to the values the editor's layout uses. So the HTML wraps lines at the same points as the canvas, and a page can show the HTML first, then swap in the live editor without the text moving.

The renderer measures text with its own copy of each font, never the system's. It bundles Atkinson Hyperlegible, which stands in for every font family until you register your own:

```rust
use canvist_headless::Renderer;

/// A renderer that measures text with your brand font, read from `path`.
fn renderer_with_font(path: &str) -> std::io::Result<Renderer> {
	let mut renderer = Renderer::new();
	// Returns the family names the font adds, such as `["Inter"]`.
	let families = renderer.register_font(std::fs::read(path)?);
	println!("registered {families:?}");
	Ok(renderer)
}
```

To change colours, fonts, and spacing, see [Theming and styling](../guides/theming.md).

## Next steps

- [Import and export](../guides/import-export.md): get your content into a document first.
- [Performance](../guides/performance.md): what loading and rendering cost.
