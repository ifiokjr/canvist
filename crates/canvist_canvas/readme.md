<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img alt="canvist" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" width="220">
    </picture>
  </a>
</p>

# canvist_canvas

The headless interaction core of [canvist](https://github.com/ifiokjr/canvist)'s canvas: everything between input and pixels, with no UI.

- **Camera:** pan, zoom about a point, momentum that does not depend on frame rate, pinch, and zoom-to-fit.
- **Scene index:** world transforms, sizes for auto-sized text and hugging frames, auto-layout, boolean outlines, and an R-tree. Each commit updates only what it touched.
- **Hit testing:** exact tests on fills (by fill rule), strokes, and curves, clipping by frames, selection targets, and marquee selection.
- **Transform handles:** resize with aspect lock, resize from the centre, and flipping; rotate with snapping; skew. Size stays separate from transform, frames apply constraints to their children, and paths rescale.
- **Snapping and rulers:** smart guides from nearby edges and centres, grid snapping, and ruler ticks for the current zoom.
- **Tools as typed extensions:** select, hand, rectangle, ellipse, polygon, frame, pen, and text, plus commands (group, ungroup, boolean, instance, image) and inspectors as data that any host or agent can read.
- **Overlay as data:** selection outlines, handles, marquee, guides, and pen previews. Draw them yourself, or hand them to the default painter.

At 100,000 nodes, a hit test takes about 0.3 µs and a drag step about 1.3 µs. Building the index takes about 75 ms. Run `cargo bench -p canvist_canvas` to reproduce.
