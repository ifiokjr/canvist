<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/ssr.md"><strong>Quick start</strong></a> ·
  <a href="https://docs.rs/canvist_layout"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_layout"><img src="https://img.shields.io/crates/v/canvist_layout?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_layout"><img src="https://img.shields.io/docsrs/canvist_layout" alt="docs.rs status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/ci.yml"><img src="https://github.com/canvist/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_layout

Incremental text and block layout for [canvist](https://github.com/canvist/canvist), and the display list every painter draws from.

Most apps add it through [`canvist`](../canvist), the crate to install, as `canvist::layout` behind the `layout` feature: `cargo add canvist --features layout`.

- **Themes:** data-driven styles for blocks, marks, and embeds, keyed by schema names so extensions add their own. The default matches TipTap's StarterKit.
- **Measurement:** a batched `TextMeasurer` trait, so one relayout costs one call to the host. Results are cached by font and word. A deterministic measurer is included for tests, and a `parley` one behind the `parley` feature.
- **Line breaking and bidi:** UAX #14 breaks via ICU4X, UAX #9 reordering, tabs, embeds, and hard breaks, following CSS `pre-wrap` rules.
- **Documents:** lists with markers, quotes, code blocks, and task checkboxes. Blocks off screen keep estimated heights until they scroll into view, and only blocks a commit touched are laid out again.
- **Queries:** grapheme- and bidi-aware hit testing, caret and selection rectangles, and a `LayoutNavigator` for moving the caret up and down.
- **Rendering:** a `DisplayList` with dirty-rectangle diffing, `to_svg`, and `to_html`, whose CSS wraps lines exactly where the canvas does.

Relaying out after a keystroke takes about 2.5 µs at any document size, and about 25 µs in the middle of a 100 KB paragraph. Opening a 10,000-paragraph document takes about 0.6 ms to the first screen. Run `cargo bench -p canvist_layout` to reproduce, or `cargo test --release -p canvist_layout --test perf_budgets -- --ignored` to check the budgets.
