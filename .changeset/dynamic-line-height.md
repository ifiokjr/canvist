---
"canvist_core": patch
---

Dynamic line height for mixed font sizes

The layout engine now computes line height dynamically from the tallest styled
run on each line, instead of using the default style's font size for all lines.

A line containing both 12px and 48px text will now have a height of 72px
(48px × 1.5 line-height multiplier) instead of 24px (16px × 1.5). This
prevents large text from overlapping adjacent lines.
