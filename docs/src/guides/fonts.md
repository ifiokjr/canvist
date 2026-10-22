# Fonts and text rendering

In this guide you choose the fonts a canvist editor uses on the web, load web fonts without layout jank, and render the same fonts on your server, so server-rendered HTML and the editor that replaces it look identical.

## Fonts work the way they do in CSS

The editor paints the document on a canvas, but it uses fonts exactly as a web page does:

- **Installed fonts work by name.** Put any family in a theme's font stack, and the editor uses it where it is installed and falls back through the stack where it isn't.
- **Web fonts your page already loads work with no setup.** Fonts declared with CSS `@font-face`, or added to `document.fonts`, are available to the editor. When text first needs a face, the editor asks the browser to load it, and only for the characters it uses, so `unicode-range` subsets download only when the text needs them. When a face finishes loading, only the blocks set in it are measured again.

Set the font stacks in the theme:

```ts no_run
import { createEditor } from "@canvist/web";

const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	await createEditor(element, {
		content: { format: "markdown", input: "# Hello\n\nThis is set in Lora." },
		theme: {
			bodyFamily: `Lora, Georgia, serif`,
			codeFamily: `"JetBrains Mono", Menlo, monospace`,
			size: 17,
		},
	});
}
```

## Load web fonts with the fonts plugin

The `fonts()` plugin from `@canvist/web/fonts` loads faces the way `@font-face` declares them, and adds what a canvas editor needs to avoid jank:

```ts no_run
import { createEditor } from "@canvist/web";
import { fonts, stack } from "@canvist/web/fonts";

const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	const editor = await createEditor(element, {
		theme: { bodyFamily: stack("Inter", "system-ui", "sans-serif") },
		plugins: [
			fonts({
				faces: [
					{
						family: "Inter",
						source: "/fonts/Inter.woff2",
						weight: "100 900",
						display: "swap",
					},
					{
						family: "Inter",
						source: "/fonts/Inter-Italic.woff2",
						weight: "100 900",
						style: "italic",
					},
				],
			}),
		],
	});

	// Load another face later, as `@font-face` would declare it.
	await editor.plugins.fonts.load({
		family: "Noto Sans JP",
		source: "/fonts/noto-sans-jp-kana.woff2",
		unicodeRange: "U+3000-30FF",
	});
}
```

- **`font-display` works as in CSS.** During a face's block period, text in it is laid out but not drawn. After that it shows in a fallback until the face arrives. A face that arrives after its swap period (`fallback`, `optional`) is never swapped in.
- **Fallbacks match the real face's metrics.** Each face gets a `"<family> Fallback"` face over a local font, scaled with `size-adjust`, `ascent-override`, and `descent-override` to the real face. Text set in the fallback then wraps nearly where the real face will, so the swap barely moves anything. `stack("Inter", …)` puts the fallback right after the family. The metrics come from the face's `metrics` option, or are measured once it loads and remembered for the next visit.
- **Discover fonts.** `available(family)` says whether a family renders as itself. `local()` lists the installed families through the Local Font Access API. Call it from a click handler: browsers ask the user for permission, and only Chromium has the API, so it returns `null` elsewhere.

Calling `editor.plugins.fonts` without registering the plugin is a type error, and the plugin's code isn't loaded unless you import it.

## Render the same fonts on your server

Server-rendered HTML wraps where the editor does only when both lay out text with the same font files. Declare the faces in the page with `fontFaceCss`, which runs anywhere, and give your server's renderer the same theme. The browser lays the HTML out with those faces, so HTML needs no fonts on the server; SVG images are laid out on the server, so register the same files there:

```ts no_run
import { fontFaceCss, type FontSource } from "@canvist/web/fonts";

const faces: FontSource[] = [{
	family: "Inter",
	source: "/fonts/Inter.ttf",
	weight: "100 900",
}];
const css = fontFaceCss(faces);
// @font-face{font-family:"Inter";src:url("/fonts/Inter.ttf");font-weight:100 900;font-display:swap}
```

On a JavaScript server, with [`@canvist/headless/ssr`](../quick-start/ssr.md):

```ts no_run
import { createRenderer } from "@canvist/headless/ssr";
import { readFile } from "node:fs/promises";

const renderer = await createRenderer({
	// The same options the browser editor's `theme` sets.
	theme: { bodyFamily: "Inter, system-ui, sans-serif" },
	// The same file the page serves, for SVG. The renderer reads TrueType
	// and OpenType, not WOFF2.
	fonts: [await readFile("public/fonts/Inter.ttf")],
});
const { html, css } = renderer.html({ format: "markdown", input: "Hello" }, {
	width: 640,
});
```

In Rust:

```rust
use std::sync::Arc;

use canvist::headless::Engine;
use canvist::headless::Format;
use canvist::headless::Renderer;
use canvist::layout::StarterKitOptions;
use canvist::layout::Theme;

fn main() -> Result<(), canvist::headless::HeadlessError> {
	// The same options the browser editor's `theme` sets.
	let theme = Theme::starter_kit_with(&StarterKitOptions {
		body_family: Arc::from("Inter, system-ui, sans-serif"),
		..StarterKitOptions::default()
	});
	let mut renderer = Renderer::with_theme(Arc::new(theme));

	// The same file the page serves. The renderer reads TrueType and
	// OpenType, not WOFF2.
	if let Ok(inter) = std::fs::read("public/fonts/Inter.ttf") {
		renderer.register_font(inter);
	}

	let doc = Engine::starter_kit().parse(Format::Markdown, "Hello")?;
	let page = renderer.html(&doc, 640.0);
	assert!(page.css.contains("font-family:Inter"));
	Ok(())
}
```

## Why the canvas matches the HTML

The editor never guesses how text looks. The browser measures every word with the same text settings the canvas paints with and the server-rendered CSS declares, so a line wraps at the same character in both. Line heights and baselines come from the browser's own line layout. Glyphs are drawn on the browser's pixel grid. canvist's tests compare the canvas with server-rendered HTML, pixel by pixel, in Chromium, Firefox, and WebKit. [Rendering parity](./rendering-parity.md) shows the results.
