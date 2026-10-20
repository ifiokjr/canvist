---
"@canvist/headless": feat
canvist_headless: feat
canvist_layout: feat
"@canvist/web": refactor
---

# Render documents on JavaScript servers with `@canvist/headless/ssr`

`@canvist/headless/ssr` renders a document as semantic HTML and a stylesheet laid out exactly as `@canvist/web` paints it, so a page shows the text before any JavaScript runs and the live editor hydrates from it without moving a pixel. It works in Node, Deno, Bun, and Workers, and so in Next.js server components and Nuxt server routes.

```ts
import { createRenderer } from "@canvist/headless/ssr";

const renderer = await createRenderer({
	theme: { bodyFamily: "Inter, sans-serif" },
});
const { html, css } = renderer.html({ format: "markdown", input: "# Hello" }, {
	width: 640,
});
```

- `renderer.html(content, { width })` returns `{ html, css }`, `renderer.page(content, { width })` a complete page, and `renderer.svg(content, { width })` an SVG image for previews. Content is text in a format or ProseMirror JSON, the same `Content` type `@canvist/web` takes.
- `createRenderer({ extensions, theme, fonts })` takes the browser editor's extensions and `ThemeOptions`, and font files for SVG text. `registerFont(bytes)` adds one later.
- Rendering is a third WebAssembly module, `wasm-ssr/`, so the document entry point stays small.
- `ThemeOptions` and `Content` are now exported by `@canvist/headless`, and `@canvist/web` re-exports them.
- `canvist_layout` gains a `json` feature with `StarterKitOptions::from_json`, which reads the theme options as both packages pass them. `canvist_headless`'s `ssr` feature enables it and re-exports `canvist_layout`.
- The `ssr_page` Rust example is gone: `examples/web` now renders its server-rendered pages with `@canvist/headless/ssr` on each request, including a React page rendered with `renderToString` and hydrated with `hydrateRoot`.
