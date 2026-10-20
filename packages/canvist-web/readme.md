<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/quick-start/web.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@canvist/web"><img src="https://img.shields.io/npm/v/%40canvist%2Fweb?color=3B2CF5" alt="npm version" /></a>
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/web.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/web.yml/badge.svg?branch=main" alt="web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/web

The canvist editor for the browser. Every pixel of the document is painted on a canvas, but the browser measures the text, so the canvas wraps each line exactly where canvist's server-rendered HTML does and hydrating moves nothing.

```sh
npm install @canvist/web
```

```ts
import { createEditor } from "@canvist/web";

const editor = await createEditor(document.querySelector("#editor")!, {
	content: { format: "markdown", input: "Hello **world**" },
});

editor.select({ path: [0], offset: 0 }, { path: [0], offset: 5 });
editor.commands.toggleItalic();
editor.toHtml(); // what a server would send: { html, css }
```

Using React or Vue? See [`@canvist/react`](../canvist-react) and [`@canvist/vue`](../canvist-vue).

## Types follow what you register

Commands, queries, and document JSON are typed from the extensions you pass, TipTap's StarterKit by default. Plugins add their own API under `editor.plugins`:

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

## Built for real typing

- **Fonts:** installed fonts and the page's web fonts work with no setup. `@canvist/web/fonts` loads faces the way `@font-face` does and adds metric-compatible fallbacks, so a font swap barely moves anything. See [Fonts and text rendering](../../docs/src/guides/fonts.md).
- **Input:** `EditContext` in Chromium and a hidden textarea elsewhere, so input methods, dictation, and autocorrect work as they do in a text field. See [Text input on the web](../../docs/src/guides/web-input.md).
- **Accessibility:** a visually hidden copy of the document lets screen readers read it.
- **Latency:** every input event is painted in its own task, and `editor.inputLatency()` reports how long that took.

<details>
<summary><strong>Development</strong></summary>

```sh
npm ci
npm run build:wasm     # builds crates/canvist_web into wasm/
npx tsc --noEmit
npm test               # unit tests in Node
npx playwright test    # parity with server-rendered HTML, editing, and input latency in Chromium, Firefox, and WebKit
node scripts/parity-report.ts
```

The parity tests and their thresholds are described in [Rendering parity](../../docs/src/guides/rendering-parity.md).

</details>
