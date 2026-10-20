# @canvist/headless

canvist's document model and editing engine for JavaScript runtimes with no DOM: Node, Deno, Bun, and Workers. Convert between ProseMirror/TipTap JSON, HTML, Markdown, and text, run typed editing commands on a server, and serve Yjs collaboration. It is the Rust core, [`canvist_headless`](../../crates/canvist_headless), compiled to WebAssembly with no `web-sys`.

**Why this name:** it matches the Rust crate it wraps, so one word means one thing across languages, and it sits under the `@canvist/*` scope the web packages (`@canvist/core`, `@canvist/react`, `@canvist/ssr`) will use. It says _no UI_ rather than _server_, because it also runs in Workers, build scripts, tests, and CLIs.

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine(); // TipTap's StarterKit
const doc = engine.parse("markdown", "Hello world");

doc.select({ path: [0], offset: 6 }, { path: [0], offset: 11 });
doc.commands.toggleBold();
doc.export("html"); // "<p>Hello <strong>world</strong></p>"
doc.toJson(); // { type: "doc", content: [...] }
```

## Types follow the extensions you register

An engine's documents only offer the registered extensions' commands, queries, blocks, and marks. The types come from the same metadata the Rust commands check their arguments against (`src/extensions.generated.ts` is generated from it), so they cannot drift:

```ts
import { bold, createEngine, heading } from "@canvist/headless";

const engine = await createEngine([bold, heading]);
const doc = engine.create();

doc.commands.setHeading({ level: 2 }); // levels are 1 | 2 | 3 | 4 | 5 | 6
doc.query.headingLevel(); // 1 | 2 | 3 | 4 | 5 | 6 | null
doc.commands.toggleItalic(); // type error: italic is not registered
doc.commands.setHeading({ level: 7 }); // type error
engine.fromJson({ type: "doc", content: [{ type: "blockquote" }] }); // type error
```

`tests/types.check.ts` pins these with `@ts-expect-error`, checked by `tsc --noEmit`.

## Positions and errors

A position is `{ path, offset }`: `path` walks the block tree from the root (`[1, 0, 0]` is the paragraph in the first item of a list that is the second block), and `offset` counts UTF-16 code units, like JavaScript strings. Every failure is an `Error` named `CanvistError` with a stable `code` (`parse`, `schema`, `invalidArgs`, `invalidPosition`, …), the same codes the Rust and Dart packages use. Narrow with `isCanvistError`.

## Collaboration

A document can be a Yjs peer and a y-websocket-compatible server endpoint:

```ts
const doc = engine.parse("markdown", "# Notes");
doc.share(1); // the server seeds the shared document

socket.send(doc.syncStart()); // when a client connects
const { reply, broadcast } = doc.receiveSync(message); // for each message
doc.validate(); // { rejectedBlocks, waiting }: content the schema does not know
```

`examples/server` is a Deno server with `POST /convert` (Markdown to JSON, or between any formats) and a `/sync/:room` WebSocket endpoint that plain `y-websocket` clients join. Its test drives it with real JavaScript Yjs clients.

```sh
deno task server
curl -X POST 'localhost:8787/convert?from=markdown&to=json' -d '# Hello **world**'
```

## Canvas scenes

`@canvist/headless/scene` loads canvist canvas scenes (pages, frames, shapes, images, vector paths, and text frames) and exports them as SVG and PNG with the renderer the canvas draws with. Use it for thumbnails, social cards, and agent-made designs. It is a separate WebAssembly module, 5.4 MB after `wasm-opt` (1.8 MB gzipped), so the document entry point stays small.

```ts
import { createSceneEngine } from "@canvist/headless/scene";

const engine = await createSceneEngine();
using doc = engine.fromJson({
	type: "canvist-scene",
	version: 1,
	nodes: [{
		type: "page",
		children: [{
			type: "rectangle",
			props: { size: [120, 80], fills: [{ type: "solid", color: "#3b2cf5" }] },
		}],
	}],
});

// Commands act on the selection, and `insert` selects what it adds.
doc.run("insert", { type: "ellipse", props: { size: [40, 40] } });
doc.setField("x", 140);
const png = doc.exportPng({ scale: 2, padding: 16 });
```

`SceneJson` types every node, property, paint, effect, and shape, so a scene an agent writes is checked before it runs. Scenes share and merge Yjs updates like documents: `share`, `encodeState`, `applyUpdate`, `takeUpdates`, and `engine.fromYjs`.

## Workers

Runtimes without a file system pass the module they imported:

```ts
import wasm from "@canvist/headless/wasm/canvist_headless_wasm_bg.wasm";
const engine = await createEngine(undefined, { wasm });
```

## Performance

`node bench/overhead.ts` measures the same operations as the Rust budgets, so the difference is the WebAssembly boundary. On Apple silicon, at 10,000 paragraphs:

| Operation                      | Rust    | Node    | Deno    | Bun     | Budget   |
| ------------------------------ | ------- | ------- | ------- | ------- | -------- |
| Run a typed command            | 0.21 µs | 0.71 µs | 0.71 µs | 1.13 µs | < 5 µs   |
| Answer a query                 | –       | 0.46 µs | 0.65 µs | 0.54 µs | < 5 µs   |
| Type or delete a character     | 0.68 µs | 1.37 µs | 1.24 µs | 2.24 µs | < 5 µs   |
| Apply one Yjs keystroke update | 2.3 µs  | 4.6 µs  | 4.9 µs  | 5.5 µs  | < 30 µs  |
| Load from Markdown             | 10 ms   | 17 ms   | 17 ms   | 18 ms   | < 100 ms |
| Export as JSON                 | 19 ms   | 22 ms   | 23 ms   | 24 ms   | < 100 ms |

The WebAssembly module is 1,058 KB after `wasm-opt` and 433 KB gzipped (budget 1,200 KB raw). Collaboration is most of it: the same core without yrs is 471 KB, or 196 KB gzipped.

## Development

```sh
npm ci
npm run build:wasm      # wasm-pack: the document module into wasm/, the scene module into wasm-scene/
npm test                # conformance and API tests in Node
deno task test          # the same, plus the server example, in Deno
npm run check:types     # tsc --noEmit, including the type-level tests
npm run generate        # regenerate the extension descriptors
npm run build           # JavaScript and declarations for npm, into dist/
```

The tests run the shared fixtures in [`conformance/`](../../conformance), which the Rust and Dart packages run too.
