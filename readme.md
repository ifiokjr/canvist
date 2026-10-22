<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="./assets/brand/vector/lockup-dark.svg">
      <img width="360" src="./assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  A <strong>rich-text editor</strong> painted on a <em>canvas</em>, with a <em>Rust</em> core.<br />
  Start with a comment box. Grow it into your own Google Docs, one plugin at a time.
</p>

<br />

<p align="center">
  <a href="#why"><strong>Why?</strong></a> ·
  <a href="./docs/src/quick-start/readme.md"><strong>Quick start</strong></a> ·
  <a href="#platforms"><strong>Platforms</strong></a> ·
  <a href="./docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="./docs/src/agents/readme.md"><strong>For agents</strong></a> ·
  <a href="./docs/src/cloud/readme.md"><strong>Cloud</strong></a> ·
  <a href="./docs/src/reference/contributing.md"><strong>Contributing</strong></a>
</p>

<br />

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/ci.yml"><img src="https://github.com/canvist/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/web.yml"><img src="https://github.com/canvist/canvist/actions/workflows/web.yml/badge.svg?branch=main" alt="Web status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/native.yml"><img src="https://github.com/canvist/canvist/actions/workflows/native.yml/badge.svg?branch=main" alt="Native status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

<br />

<p align="center">
  <img width="720" src="./docs/src/images/editor.png" alt="A canvist document with a heading, formatted text, a bullet list, a quote, a task, and a code block" />
</p>

## Why?

- **You choose every feature.** Bold, headings, lists, links, and collaboration are each an extension. Register only what your app needs, and ship only that.
- **It paints every pixel.** A Rust core lays out text and paints it to a canvas, the way Google Docs and Figma do, so it looks and behaves the same everywhere.
- **It's typed end to end.** There's no built-in toolbar. Calling a command from an extension you didn't register is a compile error, for people and AI agents alike.
- **Collaboration is built in.** Documents are Yjs-compatible, so y-websocket, Hocuspocus, and canvist Cloud all work.
- **It's fast.** A keystroke costs about 0.3 µs in the model and 2.5 µs to lay out, at 100 blocks or 100,000, and every pull request is held to those budgets.

```ts
import { bold, createEditor, heading } from "@canvist/web";

const editor = await createEditor(element, { extensions: [bold, heading] });

editor.commands.setHeading({ level: 2 });
editor.commands.toggleItalic(); // type error: italic isn't registered
```

## Platforms

One core runs everywhere, so a document exports byte for byte the same in Rust, TypeScript, and Dart.

| Platform                       | Package                                                                      | Status      |
| ------------------------------ | ---------------------------------------------------------------------------- | ----------- |
| Web                            | [`@canvist/web`](./packages/canvist-web)                                     | Available   |
| React                          | [`@canvist/react`](./packages/canvist-react)                                 | Available   |
| Vue                            | [`@canvist/vue`](./packages/canvist-vue)                                     | Available   |
| Servers: Node, Deno, Bun       | [`@canvist/headless`](./packages/canvist-headless)                           | Available   |
| Servers: Rust                  | [`canvist_headless`](./crates/canvist_headless)                              | Available   |
| Server-side rendering          | [`@canvist/headless/ssr`](./docs/src/quick-start/ssr.md), `canvist_headless` | Available   |
| Servers: Dart                  | [`canvist`](./platforms/dart)                                                | Available   |
| Flutter                        | [`canvist_flutter`](./platforms/flutter)                                     | Available   |
| Design canvas                  | [`@canvist/canvas`](./packages/canvist-canvas)                               | Available   |
| Desktop: macOS, Windows, Linux | [`canvist_desktop`](./crates/canvist_desktop)                                | Available   |
| iOS                            | [`Canvist`](./platforms/apple) Swift package                                 | Available   |
| Android                        | [`canvist-android`](./platforms/android)                                     | Available   |
| React Native and Lynx          | `@canvist/react-native`, `@canvist/lynx`                                     | Coming soon |

canvist hasn't had its first release yet. Each [quick start](./docs/src/quick-start/readme.md) shows how to install from GitHub until it does.

## Learn more

- **[Build your own editor](./docs/src/extensions/build-your-own-editor.md)**, one extension at a time.
- **[Integrate canvist with an agent](./docs/src/agents/readme.md)**, if you're an AI coding agent.
- **[canvist Cloud](./docs/src/cloud/readme.md)**: hosted real-time sync and document conversion, so you don't have to run your own servers.

## License

Released into the public domain under the [Unlicense](https://unlicense.org/).
