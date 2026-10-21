# Pick your platform

Use this page to find your platform, see what works there today, and jump to a five-minute quick start.

Every platform runs the same Rust core, so documents, extensions, and commands behave the same everywhere. A document you create in Dart exports byte for byte the same as one created in Rust or TypeScript.

## What's available today

| Platform                         | Package                                                     | Status      | Quick start                                |
| -------------------------------- | ----------------------------------------------------------- | ----------- | ------------------------------------------ |
| Server: Rust                     | `canvist_headless`                                          | Available   | [Rust](./server-rust.md)                   |
| Server: Node, Deno, Bun, Workers | `@canvist/headless`                                         | Available   | [TypeScript](./server-typescript.md)       |
| Server: Dart                     | `canvist`                                                   | Available   | [Dart](./server-dart.md)                   |
| Server-side rendering            | `@canvist/headless/ssr`, `canvist_headless` (`ssr` feature) | Available   | [SSR](./ssr.md)                            |
| Flutter                          | `canvist_flutter`                                           | Available   | [Flutter](./flutter.md)                    |
| Web                              | `@canvist/web`                                              | Preview     | [Web](./web.md)                            |
| React                            | `@canvist/react`                                            | Preview     | [React](./react.md)                        |
| Vue                              | `@canvist/vue`                                              | Preview     | [Vue](./vue.md)                            |
| iOS                              | `Canvist` Swift package                                     | Coming soon | [iOS](./ios.md)                            |
| Android                          | `canvist-android`                                           | Coming soon | [Android](./android.md)                    |
| macOS, Windows, Linux            | `canvist_desktop`                                           | Available   | [Desktop](./desktop.md)                    |
| React Native and Lynx            | `@canvist/react-native`                                     | Coming soon | [React Native and Lynx](./react-native.md) |

**Available** means the package works and its tests run on every change. canvist hasn't had its first release yet, so every quick start shows how to install from GitHub, and the registry command for after the release.

## Which one do I need?

- **You're processing content on a server:** importing Markdown, cleaning pasted HTML, migrating TipTap JSON, or running a collaboration endpoint. Use the server package for your language.
- **You're building a desktop app in Rust:** use [`canvist_desktop`](./desktop.md), a native editor component for any winit app.
- **You're building a Flutter app:** use `canvist_flutter`: `CanvistEditor` is the editor, and the same engine runs on your Dart server.
- **You want an editor in a web page:** use [`@canvist/web`](./web.md), or its [React](./react.md) and [Vue](./vue.md) bindings. Do server work with `@canvist/headless`.
- **You're an AI coding agent:** read [Integrate canvist with an agent](../agents/readme.md) first.
