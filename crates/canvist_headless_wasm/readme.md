# canvist_headless_wasm

WebAssembly bindings for [`canvist_headless`](../canvist_headless) with no DOM, built for the `@canvist/headless` package (`packages/canvist-headless`). Use that package rather than these classes directly: it loads the module in Node, Deno, Bun, and Workers and adds types generated from the extension metadata.

```sh
wasm-pack build crates/canvist_headless_wasm --target web --release --out-dir ../../packages/canvist-headless/wasm
```
