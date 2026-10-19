---
"canvist_canvas": minor
---

# Add `canvist_canvas`, the canvas's headless interaction core

`canvist_canvas` turns pointer, wheel, and key input into scene edits, with no UI of its own. Hosts feed it input and draw the scene and its overlay.

- A camera with zoom about a point, momentum, and pinch; an R-tree spatial index; and exact hit testing by fill, stroke, and text layout.
- Selection, marquee, transform handles that keep scale out of the transform, and snapping with guides.
- Tools for selection, panning, shapes, frames, the pen, and text, registered as typed extensions alongside commands and inspector fields. Machine-readable metadata describes them for bindings and agents.
- Auto-layout, constraints, and text frames laid out with `canvist_layout`.
