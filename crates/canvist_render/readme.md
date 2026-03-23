# canvist_render

> Platform-agnostic rendering traits and text layout for the canvist canvas editor.

## Overview

This crate defines the abstract rendering interface that platform backends
implement. The core editor in `canvist_core` produces layout information,
and a `Renderer` turns that into pixels.

## Traits

- **`Canvas`** — draw rectangles, text, and lines
- **`Renderer`** — viewport management
- **`TextMeasure`** — measure character and text widths

## Implementing a backend

```rust
use canvist_render::{Canvas, Rect, Renderer, Viewport};
use canvist_core::{Color, Style};

struct MyCanvas { viewport: Viewport }

impl Canvas for MyCanvas {
    fn fill_rect(&mut self, rect: Rect, color: Color) { /* draw */ }
    fn draw_text(&mut self, x: f32, y: f32, text: &str, style: &Style) { /* draw */ }
    fn clear(&mut self, color: Color) { /* clear */ }
}

impl Renderer for MyCanvas {
    fn viewport(&self) -> &Viewport { &self.viewport }
    fn set_viewport(&mut self, vp: Viewport) { self.viewport = vp; }
}
```

## Components

- **`Viewport`** — scroll position, zoom, coordinate transforms
- **`FontCache`** — fontdue-based glyph measurement
- **`Rect` / `Size`** — geometry primitives

## License

[Unlicense](https://unlicense.org/)
