# Quick start: Desktop

Use this page to see what's planned for Desktop and what you can build with canvist today.

**Status: coming soon.** The native desktop editor for macOS, Windows, and Linux isn't available yet. It will be a Rust crate that opens a window with `winit`, paints with `vello`, shapes text with `parley`, and exposes an accessibility tree through AccessKit.

## What you can do today

- **Use the engine from Rust.** [`canvist_headless`](./server-rust.md) gives a Rust app the full document model, typed commands, and every format.
- **Render documents.** The `ssr` feature [renders HTML and SVG](./ssr.md) exactly as the editor lays them out, which works for previews and thumbnails.
- **Build with Flutter.** [`canvist_flutter`](./flutter.md) runs the engine in Flutter desktop apps.

Watch [the releases](https://github.com/canvist/canvist/releases) to hear when it ships.
