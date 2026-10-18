---
"canvist_layout": minor
---

Add `canvist_layout`, the v2 layout engine.

- Data-driven `Theme` rules for blocks, marks, and embeds, with a default matching TipTap's StarterKit.
- A batched `TextMeasurer` trait with a per-(font, segment) cache, a deterministic `FixedMeasurer`, and a `ParleyMeasurer` behind the `parley` feature.
- UAX #14 line breaking with CSS `pre-wrap` semantics, UAX #9 bidi, tabs, embeds, and hard breaks.
- `DocumentLayout` with container blocks, list markers, viewport virtualization with estimated heights, and scroll anchoring.
- Incremental relayout from a `Commit`. A keystroke re-breaks only the lines around the edit, even in a 100 KB paragraph, and a structural edit re-flattens only the root subtrees it touched.
- Hit testing, caret and selection rectangles, selection bounds for anchoring host UI, and a `LayoutNavigator` for vertical caret motion that implements `canvist_editor::LineNavigator` behind the `editor` feature.
- A `DisplayList` with dirty-rectangle damage, plus `to_svg` and `to_html` renderers whose CSS reproduces the canvas line breaks.
- Release-mode performance budget tests and `divan` benchmarks at 100, 10k, and 100k blocks, plus long and heavily formatted paragraphs.
