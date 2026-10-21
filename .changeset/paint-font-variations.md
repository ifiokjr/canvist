---
"canvist_paint": fix
---

# Draw bold and italic text at the weight and slant layout measured

The GPU and CPU painters drew every glyph from a font's default instance, so bold text in a variable font (such as the system UI font) painted at regular weight, and fonts without a bold or italic face drew neither. Glyph runs now carry the variation coordinates parley shaped with, and synthetic bold and italic when the font lacks them.
