# Quick start: Server-side rendering

In this guide you render a canvist document to HTML on a server, laid out exactly as the editor paints it, then let the live editor take over in the browser without moving a pixel. Use it for fast first paint, SEO, readers without JavaScript, email, previews, and thumbnails.

**Status: available.** JavaScript servers (Node, Deno, Bun, and Workers, including Next.js and Nuxt) use `@canvist/headless/ssr`. Rust servers use `canvist_headless` with the `ssr` feature. Both render the same HTML.

## 1. Install

```sh
npm install @canvist/headless @canvist/web
```

`@canvist/headless` renders on the server, and `@canvist/web` hydrates in the browser. Rendering lives in its own WebAssembly module, so `import "@canvist/headless"` stays small.

## 2. Render HTML

```ts
import { createRenderer } from "@canvist/headless/ssr";

// Keep one renderer per server: it caches text measurements.
const renderer = await createRenderer({
	// The same theme you give the browser editor.
	theme: { bodyFamily: "Inter, sans-serif" },
});

// Semantic HTML plus a stylesheet, laid out 640 pixels wide.
const { html, css } = renderer.html(
	{ format: "markdown", input: "# Release notes\n\n- Faster **typing**" },
	{ width: 640 },
);

console.log(html.startsWith('<div class="cv-doc"'));
// true
console.log(css.includes(".cv-doc"));
// true
```

Content is text in a format (`markdown`, `html`, `text`, or `json`) or ProseMirror JSON, such as a stored `doc.toJson()`. `renderer.page(content, { width })` returns a complete HTML page instead, and `renderer.svg(content, { width })` an SVG image of the whole document for previews and thumbnails.

## 3. Hydrate in the browser

Put the HTML inside the element you pass to `createEditor`, and the stylesheet in the page:

```html
<style>/* css */</style>
<div id="editor" style="width: 640px"><!-- html --></div>
```

```ts no_run
import { createEditor } from "@canvist/web";

const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	// No `content`: the editor reads the document from the HTML.
	const editor = await createEditor(element, {
		theme: { bodyFamily: "Inter, sans-serif" },
	});
	await editor.ready(); // the canvas is showing now
}
```

The editor keeps the HTML on screen until your fonts have loaded, then swaps in the canvas in one frame. Three things must match for nothing to move:

- **The theme.** Pass the browser editor's `theme` to `createRenderer`.
- **The width.** Render at the width of the element the editor hydrates in.
- **The fonts.** Load the same font files in the page with `@font-face`. [Fonts and text rendering](../guides/fonts.md#render-the-same-fonts-on-your-server) shows how.

## Next.js

Render in a server component and hydrate in a client component. `@canvist/react`'s `CanvistEditor` takes the HTML as its `html` prop:

```tsx
// app/notes/page.tsx: a server component.
import { createRenderer } from "@canvist/headless/ssr";
import { NotesEditor } from "./notes-editor";

const THEME = { bodyFamily: "Inter, sans-serif" } as const;
// One renderer for the server: it caches text measurements.
const rendering = createRenderer({ theme: THEME });

export default async function Page() {
	const renderer = await rendering;
	const markdown = await loadNotes(); // your data
	const { html, css } = renderer.html({ format: "markdown", input: markdown }, {
		width: 640,
	});

	return (
		<>
			<style dangerouslySetInnerHTML={{ __html: css }} />
			<NotesEditor html={html} theme={THEME} />
		</>
	);
}
```

```tsx
// app/notes/notes-editor.tsx
"use client";

import { CanvistEditor } from "@canvist/react";
import type { ThemeOptions } from "@canvist/web";

export function NotesEditor(props: { html: string; theme: ThemeOptions }) {
	return (
		<CanvistEditor
			html={props.html}
			theme={props.theme}
			style={{ width: 640 }}
		/>
	);
}
```

Next.js bundles server code, so keep the WebAssembly files next to the package's modules: add `serverExternalPackages: ["@canvist/headless"]` to `next.config.ts`.

## Nuxt

Render in a server route or `useAsyncData`, and hydrate with `@canvist/vue`'s `CanvistEditor`:

```vue
<script setup lang="ts">
import { CanvistEditor } from "@canvist/vue";

const theme = { bodyFamily: "Inter, sans-serif" } as const;
// server/api/notes.get.ts renders with `@canvist/headless/ssr` and returns `{ html, css }`.
const { data } = await useFetch("/api/notes");

useHead({ style: [{ innerHTML: () => data.value?.css ?? "" }] });
</script>

<template>
	<CanvistEditor v-if="data" :html="data.html" :theme="theme" style="width: 640px" />
</template>
```

## Run the example

`examples/web` in the repository has a server (`server.ts`) that renders two pages on every request, as Next.js and Nuxt do: a plain page hydrated by `createEditor`, and a React tree rendered with `renderToString` and hydrated with `hydrateRoot`. Its tests check that hydration shifts nothing in Chromium, Firefox, and WebKit.

```sh
cd examples/web
npm install
npm run dev   # http://localhost:5180/ssr.html and /ssr-react.html
```

## Render from Rust

The same renderer is in `canvist_headless` behind the `ssr` feature:

```sh
cargo add canvist_headless --git https://github.com/canvist/canvist --features ssr
```

```rust
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Renderer;

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let doc =
		Engine::starter_kit().parse(Format::Markdown, "# Release notes\n\n- Faster **typing**")?;

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

`Renderer::with_theme` takes the theme, `Theme::starter_kit_with(&StarterKitOptions { .. })`, with the same options as the browser editor's `theme`.

## How the HTML matches the editor

The stylesheet pins line breaking, fonts, and spacing to the values the editor's layout uses. So the browser wraps the HTML at the same points as the canvas, and the live editor can replace it without the text moving.

HTML needs no fonts on the server, because the browser lays it out. SVG images are laid out on the server, so the renderer measures their text with its own copy of each font, never the system's. It bundles Atkinson Hyperlegible, which stands in for every font family until you register your own:

```ts no_run
import { createRenderer } from "@canvist/headless/ssr";
import { readFile } from "node:fs/promises";

const renderer = await createRenderer({
	theme: { bodyFamily: "Inter, sans-serif" },
	// The files your page's `@font-face` rules load.
	fonts: [await readFile("fonts/Inter.ttf")],
});
const svg = renderer.svg({ format: "markdown", input: "# Hello" }, {
	width: 640,
});
```

`renderer.registerFont(bytes)` adds one later and returns the family names it added. In Rust, call `renderer.register_font(bytes)`. To change colours, fonts, and spacing, see [Theming and styling](../guides/theming.md).

## Next steps

- [Import and export](../guides/import-export.md): get your content into a document first.
- [Performance](../guides/performance.md): what loading and rendering cost.
