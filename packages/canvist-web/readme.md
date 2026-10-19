# @canvist/web

The canvist editor for the browser. Every pixel of the document is painted on a canvas, but text is measured by the browser itself: the canvas wraps each line exactly where canvist's server-rendered HTML does, so hydrating from SSR moves nothing. It is the Rust core, [`canvist_web`](../../crates/canvist_web), compiled to WebAssembly.

```ts
import { createEditor } from "@canvist/web";

const editor = await createEditor(document.querySelector("#editor")!, {
	content: { format: "markdown", input: "Hello **world**" },
});

editor.select({ path: [0], offset: 0 }, { path: [0], offset: 5 });
editor.commands.toggleItalic();
editor.toHtml(); // what a server would send: { html, css }
```

## Types follow what you register

Commands, queries, and document JSON are typed from the extensions you pass (TipTap's StarterKit by default), with the same generated descriptors as [`@canvist/headless`](../canvist-headless). Plugins add their own API under `editor.plugins`. Using an extension or plugin you did not register is a type error:

```ts
import { bold, createEditor, heading } from "@canvist/web";
import { fonts } from "@canvist/web/fonts";

const editor = await createEditor(element, {
	extensions: [bold, heading],
	plugins: [fonts()],
});

editor.commands.setHeading({ level: 2 });
editor.commands.toggleItalic(); // type error: italic is not registered
await editor.plugins.fonts.local(); // installed fonts, where the browser allows it
```

## Fonts

Installed fonts and every web font the page already declares work with no configuration. `@canvist/web/fonts` loads faces the way `@font-face` does (URLs or bytes, `unicode-range`, `font-display`), adds metric-compatible fallbacks so a font swap barely moves anything, lists local fonts through the Local Font Access API, and writes the same faces as `@font-face` CSS for server rendering. See [Fonts and text rendering](../../docs/src/guides/fonts.md).

## Development

```sh
npm ci
npm run build:wasm     # builds crates/canvist_web into wasm/
npx tsc --noEmit
npm test               # unit tests in Node
npx playwright test    # pixel parity with server-rendered HTML in Chromium, Firefox, and WebKit
node scripts/parity-report.ts
```

The parity tests and their thresholds are described in [Rendering parity](../../docs/src/guides/rendering-parity.md).
