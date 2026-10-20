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
  <a href="https://github.com/canvist/canvist/actions/workflows/headless-js.yml"><img src="https://github.com/canvist/canvist/actions/workflows/headless-js.yml/badge.svg?branch=main" alt="headless-js status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_headless_wasm

WebAssembly bindings for [`canvist_headless`](../canvist_headless) with no DOM, built for the `@canvist/headless` package (`packages/canvist-headless`). Use that package rather than these classes directly: it loads the module in Node, Deno, Bun, and Workers and adds types generated from the extension metadata.

```sh
wasm-pack build crates/canvist_headless_wasm --target web --release --out-dir ../../packages/canvist-headless/wasm
```
