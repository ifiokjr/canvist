<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/server-typescript.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@canvist/headless"><img src="https://img.shields.io/npm/v/%40canvist%2Fheadless?color=3B2CF5" alt="npm version" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/headless-js.yml"><img src="https://github.com/canvist/canvist/actions/workflows/headless-js.yml/badge.svg?branch=main" alt="headless-js status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/headless

canvist's document model and editing engine for JavaScript with no DOM: Node, Deno, Bun, and Workers. Convert between ProseMirror/TipTap JSON, HTML, Markdown, and text, run typed editing commands, and serve Yjs collaboration from the same Rust core the editor uses, compiled to WebAssembly.

```sh
npm install @canvist/headless
```

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine(); // TipTap's StarterKit
const doc = engine.parse("markdown", "Hello world");

doc.select({ path: [0], offset: 6 }, { path: [0], offset: 11 });
doc.commands.toggleBold();
doc.export("html"); // "<p>Hello <strong>world</strong></p>"
```

## Types follow your extensions

A document only offers the commands, queries, blocks, and marks of the extensions you registered:

```ts
import { bold, createEngine, heading } from "@canvist/headless";

const engine = await createEngine([bold, heading]);
const doc = engine.create();

doc.commands.setHeading({ level: 2 }); // levels are 1 | 2 | 3 | 4 | 5 | 6
doc.commands.toggleItalic(); // type error: italic is not registered
doc.commands.setHeading({ level: 7 }); // type error
```

## What else it does

- **Positions:** `{ path, offset }`, where `path` walks the block tree and `offset` counts UTF-16 code units, like JavaScript strings. Errors are `CanvistError`s with the same stable `code`s as the Rust and Dart packages.
- **Collaboration:** a document can be a Yjs peer and a y-websocket-compatible server endpoint (`share`, `syncStart`, `receiveSync`). [`examples/server`](./examples/server) serves `POST /convert` and a `/sync/:room` WebSocket in a few lines.
- **Server-side rendering:** `@canvist/headless/ssr` renders a document as HTML and CSS laid out exactly as `@canvist/web` paints it, so the live editor hydrates from it without moving a pixel, and as SVG for previews. It's a separate WebAssembly module too. See [Server-side rendering](../../docs/src/quick-start/ssr.md).
- **Canvas scenes:** `@canvist/headless/scene` loads canvist canvas scenes and exports SVG and PNG, for thumbnails, social cards, and designs an agent writes. It's a separate WebAssembly module, so the document entry point stays small.
- **Workers:** runtimes without a file system pass the module they imported: `createEngine(undefined, { wasm })`.

A typed command costs about 0.7 µs in Node and a collaborator's keystroke about 4.6 µs, at 10,000 paragraphs. `node bench/overhead.ts` reproduces the numbers.

<details>
<summary><strong>Development</strong></summary>

```sh
npm ci
npm run build:wasm      # the document, scene, and rendering modules into wasm/, wasm-scene/, and wasm-ssr/
npm test                # conformance and API tests in Node
deno task test          # the same, plus the server example, in Deno
npm run check:types     # tsc --noEmit, including the type-level tests
npm run generate        # regenerate the extension descriptors
npm run build           # JavaScript and declarations for npm, into dist/
```

The tests run the shared fixtures in [`conformance/`](../../conformance), which the Rust and Dart packages run too.

</details>
