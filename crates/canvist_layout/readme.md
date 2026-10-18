# canvist_layout

Incremental text and block layout for [canvist](https://github.com/ifiokjr/canvist), and the display list every painter draws from.

- **Themes:** data-driven styles for blocks, marks, and embeds, keyed by schema names so extensions add their own. The default matches TipTap's StarterKit.
- **Measurement:** a batched `TextMeasurer` trait, so one relayout costs one call to the host. Results are cached by font and word. A deterministic measurer is included for tests, and a `parley` one behind the `parley` feature.
- **Line breaking and bidi:** UAX #14 breaks via ICU4X, UAX #9 reordering, tabs, embeds, and hard breaks, following CSS `pre-wrap` rules.
- **Documents:** lists with markers, quotes, code blocks, and task checkboxes. Blocks off screen keep estimated heights until they scroll into view, and only blocks a commit touched are laid out again.
- **Queries:** grapheme- and bidi-aware hit testing, caret and selection rectangles, and a `LineNavigator` for moving the caret up and down.
- **Rendering:** a `DisplayList` with dirty-rectangle diffing, `to_svg`, and `to_html`, whose CSS wraps lines exactly where the canvas does.
