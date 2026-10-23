<h1 class="brand-title">
  <img class="brand-light" src="images/brand/lockup.svg" alt="canvist">
  <img class="brand-dark" src="images/brand/lockup-dark.svg" alt="canvist">
</h1>

**canvist is a rich-text editor you build from plugins.** Start with plain paragraphs. Add bold, then headings, lists, links, and live collaboration. Stop wherever your app needs to stop, or keep going until you have your own Google Docs.

![A canvist document with a heading, formatted text, a bullet list, a quote, a task, and a code block](images/editor.png)

## Why canvist

- **You choose every feature.** Each feature is an extension: bold, headings, lists, tasks, links, and more. Register the ones you want and nothing else gets in. A comment box can be paragraphs and bold. A notes app can be the full StarterKit.
- **It draws every pixel itself.** canvist doesn't use `contenteditable` or the platform's text view. A Rust core lays the text out and paints it to a canvas, the way Google Docs and Figma do. So the editor looks and behaves the same everywhere.
- **One core, every platform.** The same engine runs in the browser (with React and Vue), in React Native, Flutter, iOS, Android, desktop, and Lynx apps, and on servers (TypeScript, Rust, Dart). [Check the status of your platform](./quick-start/readme.md).
- **Collaboration is built in.** Documents are [Yjs](https://yjs.dev)-compatible. Connect them to y-websocket, Hocuspocus, or [canvist Cloud](./cloud/readme.md).
- **Typed, headless APIs.** canvist ships no toolbar. You call typed commands and build your own UI on top. If you call a command from an extension you didn't register, your code doesn't compile. That catches the mistakes people and AI agents make most.
- **Free forever.** The code is released into the public domain under the [Unlicense](https://unlicense.org).

## See it in ten lines

This TypeScript runs on a server, in a script, or in a test. It registers two extensions, loads Markdown, makes a word bold, and exports HTML:

```ts
import { bold, createEngine, heading } from "@canvist/headless";

const engine = await createEngine([bold, heading]);
const doc = engine.parse("markdown", "# Notes\n\nShip it today");

doc.select({ path: [1], offset: 8 }, { path: [1], offset: 13 });
doc.commands.toggleBold();

console.log(doc.export("html"));
// <h1>Notes</h1><p>Ship it <strong>today</strong></p>
```

`doc.commands.toggleItalic()` would be a type error here, because italic isn't registered.

## Where to go next

- **[Quick start](./quick-start/readme.md):** install canvist on your platform and get a working document in five minutes.
- **[Core concepts](./concepts/documents.md):** documents, extensions, commands, and selections, in a few short pages.
- **[Build your own editor](./extensions/build-your-own-editor.md):** go from a minimal editor to a Google Docs-style one, one extension at a time.
- **[Integrate canvist with an agent](./agents/readme.md):** the steps and checklist for AI coding agents.
