---
"canvist_paint": feat
---

# Paint any display list, and shape with the system's fonts

- `paint_display_list` paints a `canvist_layout` display list through any `Painter`, with a `DisplayStyle` transform and optional fill. Canvas text frames now paint through it too, and `GREEK_PIXELS` moves with it.
- `Shaper::with_system_fonts` reads the fonts installed on the machine, for desktop hosts.
- `ShapedRun` gains `coords`, `embolden`, and `skew`, with `embolden_amount()` and `shear()`.
