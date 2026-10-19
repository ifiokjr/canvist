# Quick start: TypeScript

In this guide you add canvist to a Node, Deno, Bun, or Workers project, then load, edit, and export a document with typed commands. It takes about five minutes.

`@canvist/headless` is canvist's engine compiled to WebAssembly, with no DOM. Use it in API routes, build scripts, CLIs, tests, and background jobs.

**Status: available.** The package works today. It goes to npm and JSR with the first release; until then, build it from source.

## 1. Install

After the first release:

```sh
npm install @canvist/headless      # Node and Bun
deno add jsr:@canvist/headless     # Deno
```

Until then, build the package once and install the tarball. You need Node 22 or newer, Rust, and [wasm-pack](https://rustwasm.github.io/wasm-pack/):

```sh
git clone https://github.com/ifiokjr/canvist
cd canvist/packages/canvist-headless
npm ci && npm run build:wasm && npm pack
cd /path/to/your/app
npm install /path/to/canvist/packages/canvist-headless/canvist-headless-0.1.0.tgz
```

## 2. Load, edit, and export a document

Create `main.ts`:

```ts
import { createEngine } from "@canvist/headless";

// The StarterKit: paragraphs, headings, lists, tasks, quotes, code, marks, links.
const engine = await createEngine();
const doc = engine.parse("markdown", "# Plan\n\nShip the docs");

// Select "docs" (block 1, characters 9 to 13) and make it bold.
doc.select({ path: [1], offset: 9 }, { path: [1], offset: 13 });
doc.commands.toggleBold();

console.log(doc.query.isBold());
// true
console.log(doc.export("markdown"));
// # Plan
//
// Ship the **docs**
```

Run it:

```sh
node main.ts         # Node 24 runs TypeScript directly
deno run main.ts     # or Deno
bun main.ts          # or Bun
```

## 3. Pick only the extensions you need

Pass the extensions you want to `createEngine`. Every document from that engine only has their commands, queries, blocks, and marks:

```ts
import { bold, createEngine, heading } from "@canvist/headless";

const engine = await createEngine([bold, heading]);
const doc = engine.create();

doc.commands.setHeading({ level: 2 });
console.log(doc.query.headingLevel());
// 2

// Neither of these compiles. `@ts-expect-error` marks the errors.
function mistakes() {
	// @ts-expect-error: italic isn't registered.
	doc.commands.toggleItalic();
	// @ts-expect-error: headings go from 1 to 6.
	doc.commands.setHeading({ level: 7 });
}
```

The types come from the same metadata the engine checks arguments against at runtime, so they can't drift. Code that skips the types, such as JavaScript, still gets a `CanvistError` with the code `invalidArgs`.

## Workers and other runtimes without a file system

In Cloudflare Workers and similar runtimes, import the WebAssembly module and pass it in:

```ts no_run
import { createEngine } from "@canvist/headless";
import wasm from "@canvist/headless/wasm/canvist_headless_wasm_bg.wasm";

export default {
	async fetch(request: Request): Promise<Response> {
		const engine = await createEngine(undefined, { wasm });
		const doc = engine.parse("markdown", await request.text());
		return Response.json(doc.toJson());
	},
};
```

`createEngine` loads the module only once, so calling it per request is cheap after the first.

## Next steps

- [Import and export](../guides/import-export.md) JSON, HTML, Markdown, and text.
- [Collaboration](../guides/collaboration.md): run a y-websocket-compatible endpoint in about 30 lines.
- [Commands and typed handles](../concepts/commands.md): run, check, and query commands.
