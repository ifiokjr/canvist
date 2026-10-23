# Pick your platform

Use this page to find your platform, see what works there today, and jump to a five-minute quick start.

Every platform runs the same Rust core, so documents, extensions, and commands behave the same everywhere. A document you create in Dart exports byte for byte the same as one created in Rust or TypeScript.

## What's available today

| Platform                         | Package                                            | Status    | Quick start                          |
| -------------------------------- | -------------------------------------------------- | --------- | ------------------------------------ |
| React                            | `@canvist/react`                                   | Available | [React](./react.md)                  |
| React Native                     | `@canvist/react-native`                            | Preview   | [React Native](./react-native.md)    |
| Vue                              | `@canvist/vue`                                     | Available | [Vue](./vue.md)                      |
| Web                              | `@canvist/web`                                     | Available | [Web](./web.md)                      |
| Server-side rendering            | `@canvist/headless/ssr`, `canvist` (`ssr` feature) | Available | [SSR](./ssr.md)                      |
| Server: Node, Deno, Bun, Workers | `@canvist/headless`                                | Available | [TypeScript](./server-typescript.md) |
| Design canvas                    | `@canvist/canvas`                                  | Available | [Design canvas](./canvas.md)         |
| Flutter                          | `canvist_flutter`                                  | Available | [Flutter](./flutter.md)              |
| Server: Dart                     | `canvist`                                          | Available | [Dart](./server-dart.md)             |
| iOS                              | `Canvist` Swift package                            | Available | [iOS](./ios.md)                      |
| Android                          | `canvist-android`                                  | Available | [Android](./android.md)              |
| macOS, Windows, Linux            | `canvist_desktop`                                  | Available | [Desktop](./desktop.md)              |
| Server: Rust                     | `canvist`                                          | Available | [Rust](./server-rust.md)             |
| Lynx                             | `@canvist/lynx`                                    | Preview   | [Lynx](./lynx.md)                    |

**Available** means the package works and its tests run on every change. canvist hasn't had its first release yet, so every quick start shows how to install from GitHub, and the registry command for after the release. **Preview** means the same, except the package stays on GitHub after the first release: React Native and Lynx build on the iOS and Android editors, which aren't on CocoaPods or Maven Central yet.

## Which one do I need?

- **You want an editor in a web page:** use [`@canvist/web`](./web.md), or its [React](./react.md) and [Vue](./vue.md) bindings. Render it on your server with [`@canvist/headless/ssr`](./ssr.md), and do other server work with `@canvist/headless`.
- **You're building a React Native app:** use [`@canvist/react-native`](./react-native.md), which shows the native iOS and Android editors as a Fabric component.
- **You want a design canvas, like Figma's:** use [`@canvist/canvas`](./canvas.md).
- **You're building a Flutter app:** use `canvist_flutter`: `CanvistEditor` is the editor, and the same engine runs on your Dart server.
- **You're building an iOS app:** use the [`Canvist` Swift package](./ios.md). `CanvistView` is a native UIKit editor, with `CanvistEditor` for SwiftUI.
- **You're building an Android app:** use [`canvist-android`](./android.md). `CanvistView` is a native editor for Views and Jetpack Compose.
- **You're building a desktop app in Rust:** use [`canvist_desktop`](./desktop.md), a native editor component for any winit app.
- **You're building a Lynx app:** use [`@canvist/lynx`](./lynx.md), the same native editors as a `<canvist-editor>` element.
- **You're processing content on a server:** importing Markdown, cleaning pasted HTML, migrating TipTap JSON, or running a collaboration endpoint. Use the server package for your language.
- **You're an AI coding agent:** read [Integrate canvist with an agent](../agents/readme.md) first.
