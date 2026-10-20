<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/web.yml"><img src="https://github.com/canvist/canvist/actions/workflows/web.yml/badge.svg?branch=main" alt="web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_web

The canvist editor's WebAssembly half for the browser: a `canvist_headless` document laid out by [`canvist_layout`](../canvist_layout) and painted on an opaque Canvas2D context. It is built for the `@canvist/web` package (`packages/canvist-web`). Use that package rather than this crate directly: it owns sizing, scrolling, the frame loop, input, and fonts, and adds types generated from the extension metadata.

Text is measured by the browser itself, through the package's `CanvasMeasurer`, so the canvas wraps every line where canvist's server-rendered HTML does. See [Fonts and text rendering](../../docs/src/guides/fonts.md).

```sh
wasm-pack build crates/canvist_web --target web --profile wasm-web --out-dir ../../packages/canvist-web/wasm --no-pack
```
