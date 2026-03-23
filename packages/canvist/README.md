# @canvist/canvist

> A canvas-based text editor for the browser — build your own Google Docs.

**canvist** renders text through a custom WASM-powered canvas engine, giving you full control over every pixel. Instead of relying on `contenteditable`, it manages its own rendering, selection, input handling, and accessibility.

## Features

- 🎨 **Canvas rendering** — platform-agnostic, pixel-perfect text rendering
- ✨ **Rich formatting** — bold, italic, underline, strikethrough, font size, colors
- ↩️ **Smart undo** — word-at-a-time coalescing with configurable timeout
- 🤝 **Real-time collaboration** — built-in Yrs CRDT with encode/decode/sync API
- ♿ **Accessibility** — hidden DOM mirror for screen readers + announcements
- 📋 **Rich paste** — paste HTML preserving formatting
- 🌐 **Cross-platform** — WebAssembly runs anywhere

## Install

```bash
# Deno / JSR
deno add @canvist/canvist

# npm
npx jsr add @canvist/canvist
```

## Quick start

```ts
import { createEditor } from "@canvist/canvist";

const editor = await createEditor("my-canvas");

// Insert and format text.
editor.insertText("Hello, canvist!");
editor.render();

// Bold a range.
editor.setSelection(0, 5);
editor.toggleBold();
editor.render();

console.log(editor.text);      // "Hello, canvist!"
console.log(editor.charCount);  // 15
console.log(editor.toHtml());   // "<p><strong>Hello</strong>, canvist!</p>"
```

## Collaboration

```ts
const editor = await createEditor("canvas");

// Enable CRDT session.
editor.enableCollab();

// Send state to a new peer.
const state = editor.collabEncodeState(); // Uint8Array
sendToPeer(state);

// Apply remote updates.
onPeerUpdate((update: Uint8Array) => {
  editor.collabApplyUpdate(update);
  editor.render();
});

// After local edits, sync to CRDT.
editor.collabSyncLocal();
```

## API highlights

| Property / Method        | Description                              |
| ------------------------ | ---------------------------------------- |
| `text`                   | Current plain-text content               |
| `charCount`              | Number of characters                     |
| `insertText(text)`       | Insert text at cursor                    |
| `deleteRange(start,end)` | Delete a character range                 |
| `toggleBold()`           | Toggle bold (works with selection or pending) |
| `toggleItalic()`         | Toggle italic                            |
| `toggleUnderline()`      | Toggle underline                         |
| `setTextAlign(align)`    | Set alignment: left, center, right       |
| `undo()` / `redo()`      | Undo/redo with coalescing                |
| `toHtml()` / `toMarkdown()` | Export as HTML or Markdown           |
| `toJSON()` / `fromJSON()`   | Serialize/deserialize document       |
| `enableCollab()`         | Start a Yrs CRDT collaboration session   |
| `render()`               | Re-render the canvas                     |
| `destroy()`              | Release WASM resources                   |

## Demos

- **Editor demo**: `packages/canvist/demo/index.html`
- **Collaboration demo**: `packages/canvist/demo/collab.html` (tab-to-tab sync via BroadcastChannel)

## Development

```bash
# Build WASM
deno task build:wasm

# Run unit tests
deno task test:unit

# Run browser tests (Playwright)
deno task test:playwright
```

## License

[Unlicense](https://unlicense.org/)
