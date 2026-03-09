# @canvist/canvist

> A canvas-based text editor for the browser — build your own Google Docs.

**canvist** renders text through a custom WASM-powered canvas engine, giving you full control over every pixel. Instead of relying on `contenteditable`, it manages its own rendering, selection, input handling, and accessibility.

## Install

```bash
# Deno / JSR
deno add @canvist/canvist

# npm
npx jsr add @canvist/canvist
```

## Usage

```ts
import { createEditor } from "@canvist/canvist";

const editor = await createEditor("my-canvas");
editor.insertText("Hello, canvist!");
editor.render();

console.log(editor.text); // "Hello, canvist!"
console.log(editor.charCount); // 15
```

## API

### `createEditor(canvasId: string, options?: EditorOptions): Promise<CanvistEditor>`

Create an editor attached to a `<canvas>` element. Initialises the WASM module automatically on first call.

### `initWasm(): Promise<void>`

Explicitly initialise the WASM module. Idempotent — safe to call multiple times.

### `CanvistEditor`

| Property / Method  | Description                |
| ------------------ | -------------------------- |
| `text`             | Current plain-text content |
| `charCount`        | Number of characters       |
| `canvasId`         | The canvas element ID      |
| `insertText(text)` | Insert text at cursor      |
| `setTitle(title)`  | Set document title         |
| `render()`         | Re-render to canvas        |
| `toJSON()`         | Export document as JSON    |
| `destroy()`        | Release resources          |

## Development

From `packages/canvist`, use Deno tasks so local runs match CI:

```bash
# Build the WebAssembly bundle used by the package.
deno task build:wasm

# Run non-browser tests.
deno task test:unit

# Run browser E2E tests (Playwright). Includes --allow-sys for OS detection and --allow-write for local browser/temp artifacts.
# Playwright specs use *.spec.ts so they stay out of Deno's default *_test.ts unit discovery.
deno task test:playwright

# Run the full web test matrix used by CI.
deno task ci:test
```

## License

[Unlicense](https://unlicense.org/)
