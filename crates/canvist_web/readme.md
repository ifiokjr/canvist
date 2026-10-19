# canvist_web

The canvist editor's WebAssembly half for the browser: a `canvist_headless` document laid out by [`canvist_layout`](../canvist_layout) and painted on an opaque Canvas2D context. It is built for the `@canvist/web` package (`packages/canvist-web`). Use that package rather than this crate directly: it owns sizing, scrolling, the frame loop, input, and fonts, and adds types generated from the extension metadata.

Text is measured by the browser itself, through the package's `CanvasMeasurer`, so the canvas wraps every line where canvist's server-rendered HTML does. See [Fonts and text rendering](../../docs/src/guides/fonts.md).

```sh
wasm-pack build crates/canvist_web --target web --profile wasm-web --out-dir ../../packages/canvist-web/wasm --no-pack
```
