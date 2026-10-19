---
"canvist_headless_wasm": minor
---

Add `@canvist/headless` (`packages/canvist-headless`), canvist's document model for JavaScript runtimes with no DOM, over a new `canvist_headless_wasm` crate (wasm-bindgen, no `web-sys`).

- Loads in Node, Deno, and Bun from disk, and in Workers from an imported `WebAssembly.Module`. A `build` step emits JavaScript for npm, and CI installs the packed tarball as a dependency.
- Types come from the extension metadata (`src/extensions.generated.ts`, generated from the `canvist_extensions` snapshot): an engine's documents only offer the registered extensions' commands, queries, blocks, and marks, with argument types such as heading levels `1 | … | 6`. `tests/types.check.ts` pins this with `tsc --noEmit` and `@ts-expect-error`.
- The shared conformance fixtures pass in Node and Deno (30 of 30), alongside API tests that sync with JavaScript Yjs.
- `examples/server` is a Deno server that converts Markdown to JSON over HTTP and runs a y-websocket-compatible sync endpoint, tested end to end with real Yjs clients.
- `bench/overhead.ts` records per-call overhead and the wasm size against budgets.
