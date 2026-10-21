<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/desktop.md"><strong>Quick start</strong></a> ·
  <a href="https://docs.rs/canvist_desktop"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_desktop"><img src="https://img.shields.io/crates/v/canvist_desktop?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_desktop"><img src="https://img.shields.io/docsrs/canvist_desktop" alt="docs.rs status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/desktop.yml"><img src="https://github.com/canvist/canvist/actions/workflows/desktop.yml/badge.svg?branch=main" alt="desktop status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_desktop

The [canvist](https://github.com/canvist/canvist) rich-text editor for macOS, Windows, and Linux: a native component for any [winit](https://github.com/rust-windowing/winit) app, painted with [vello](https://github.com/linebender/vello) and laid out by the same engine as canvist's server-side renderer, so a document wraps identically in the window, in a PNG, and in HTML.

- **Typing like a text view:** key bindings with Cmd on macOS and Ctrl elsewhere, input rules (`##` makes a heading), undo grouped by typing bursts, and paging.
- **Input methods:** preedit text is underlined in place, and the candidate window sits under it.
- **Clipboard:** copy and cut write HTML and plain text; paste prefers HTML, so formatting survives other apps.
- **Mouse and trackpad:** click, double-click for a word, triple-click for a block, Shift-click and drag to extend, and autoscroll past the edges.
- **Scrolling and high-DPI**, with only the visible blocks laid out and painted.
- **Accessibility:** an [AccessKit](https://accesskit.dev) tree of text runs with character positions, so screen readers read, navigate, and select text.
- **Fonts:** the system's own, plus any loaded from bytes with `register_font`.
- **Typed commands:** handles from the engine's extensions, so an unregistered extension's commands don't compile.
- **Collaboration** (`collab` feature): join a y-websocket or Hocuspocus room and edit live.

```rust no_run
use canvist_desktop::DesktopEditor;
use canvist_desktop::EditorOptions;
use canvist_desktop::winit::winit::window::Window;
use canvist_headless::Engine;
use canvist_headless::Format;

let mut editor = DesktopEditor::new(Engine::starter_kit(), EditorOptions::new().max_width(720.0))?;
editor.load(Format::Markdown, "# Hello\n\nStart typing.")?;
canvist_desktop::winit::run(editor, Window::default_attributes().with_title("Notes"))?;
# Ok::<(), canvist_desktop::DesktopError>(())
```

To put the editor in your own app beside a toolbar, use `winit::EditorWindow`: see the [quick start](../../docs/src/quick-start/desktop.md). Without the default `winit` feature, the crate is the headless editor core: it takes input as `InputEvent`s, lays out, renders PNGs, and builds the accessibility tree, which is how its tests drive it.

A keystroke's whole frame (the edit, layout, display list, and GPU scene encoding) takes about 0.14 ms in a 10,000-paragraph document on an Apple silicon Mac. The [performance budgets](../../docs/src/guides/performance.md) hold it under 4 ms.
