# canvist_render

> Platform-agnostic rendering traits and text layout for the canvist canvas editor.

This crate defines the abstract rendering interface that platform backends
implement. It provides:

- **`Renderer` trait** — the contract every rendering backend must fulfil
- **`Canvas` trait** — drawing primitives (rectangles, text, paths)
- **`Viewport`** — scroll position and visible area management
- **Font loading** — via fontdue for software rasterisation
- **Accessibility** — tree generation for parallel hidden DOM
