<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/brand/vector/lockup-dark.svg">
    <img alt="canvist" src="./assets/brand/vector/lockup.svg" width="320">
  </picture>
</h1>

> A rich-text editor you build from plugins. Add bold, headings, lists, links, and live collaboration, as little or as much as you want, up to your own Google Docs.

[![ci](https://github.com/ifiokjr/canvist/actions/workflows/ci.yml/badge.svg)](https://github.com/ifiokjr/canvist/actions/workflows/ci.yml)

![A canvist document with a heading, formatted text, a bullet list, a quote, a task, and a code block](./docs/src/images/editor.png)

- **You choose every feature.** Each one is an extension. Register only what your app needs.
- **It paints every pixel.** A Rust core lays out text and paints it to a canvas, like Google Docs and Figma, so it looks and behaves the same everywhere.
- **One core, every platform.** Servers (Rust, TypeScript, Dart) and Flutter today; web, React, Vue, iOS, Android, desktop, React Native, and Lynx on the way.
- **Collaboration built in.** Documents are Yjs-compatible: y-websocket, Hocuspocus, or canvist Cloud.
- **Typed, headless APIs.** No built-in toolbar. Calling a command from an extension you didn't register is a compile error, for people and AI agents alike.

```ts
import { bold, createEngine, heading } from "@canvist/headless";

const engine = await createEngine([bold, heading]);
const doc = engine.parse("markdown", "# Notes\n\nShip it today");

doc.select({ path: [1], offset: 8 }, { path: [1], offset: 13 });
doc.commands.toggleBold();

doc.export("html"); // <h1>Notes</h1><p>Ship it <strong>today</strong></p>
```

## Get started

- **[Pick your platform](./docs/src/quick-start/readme.md)** and get a working document in five minutes.
- **[Build your own editor](./docs/src/extensions/build-your-own-editor.md)**, one extension at a time.
- **[Integrate canvist with an agent](./docs/src/agents/readme.md)**, if you're an AI coding agent.
- **[Read the docs](./docs/src/readme.md)**: concepts, the extension catalogue, guides, and canvist Cloud.

canvist hasn't had its first release yet. Every quick start shows how to install from GitHub.

## Contributing

See [Contributing](./docs/src/reference/contributing.md).

## License

Released into the public domain under the [Unlicense](https://unlicense.org/).
