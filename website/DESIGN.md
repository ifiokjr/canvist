# canvist website design

Recorded from the built site (October 2026). [PRODUCT.md](./PRODUCT.md) holds the product facts; this file holds the visual system. Tokens live in [`src/styles/tokens.css`](./src/styles/tokens.css) and the chip system in [`src/styles/global.css`](./src/styles/global.css).

## The world: a paint fan deck

The site is a fan deck of canvist's own colour. Every landing section is a swatch chip: a full-width rounded card punched with a rivet hole in its top-left corner, with a label strip along its bottom (`No. 04 · Together · Apply a remote keystroke: 2.7 µs @ 100k blocks`). Label codes are always real numbers from that section, never decoration.

As you scroll, each chip swings down from its rivet and settles flat. The hero shows the rest of the deck fanned open behind the wordmark, and the fan doubles as section navigation.

## Colour

- **Brand:** Ink `#17141F`, Paint `#3B2CF5`, Paper `#F5F3FF`, Paint on dark `#8A80FF` (from `assets/brand/readme.md`).
- **The ramp:** `--chip-0` to `--chip-7` step from Paper through Paint tints to Ink in light mode, and lift from Ink to pale violet in dark mode. `--chip-4` is Paint in both themes. Each section takes the next chip, so the page darkens (or lightens) as it goes, like paint drying.
- **Text on chips:** `--chip-fg`, and `--chip-muted` (86% of the foreground mixed into the chip) for secondary text. That is the most a muted tone can fade and still pass WCAG AA on every chip; don't go lower.
- **The caret:** only Paint, Paint on dark, or one mono colour (Paper on a Paint chip), per the brand rules.
- **Semantic exceptions:** `--error` for type errors and over-budget values. The four presence colours in the Together demo (`#3B2CF5`, `#E5484D`, `#0E9384`, `#C07A00`) are the only other hues.
- **Themes:** one `light-dark()` declaration per token, driven by `color-scheme`. `data-theme` on `<html>` pins a theme and the toggle stores it.

## Type

- **Bricolage Grotesque** (variable, optical size and weight) for everything that isn't code. Display uses weights 750–800 with tight tracking (`-0.035em` to `-0.05em`); body copy uses the text optical size at 17 px with a 1.55 line height.
- **Geist Mono** for swatch codes, eyebrows, readouts and code.
- Both are subset to the characters the copy uses (`deno task fonts`) and load with `font-display: optional`, so they never shift layout.
- Scale: `--t-mega` (hero numerals, closing titles), `--t-1` (section titles), `--t-2`, `--t-3` (ledes), body, `--t-small`, `--t-micro`.
- The wordmark is never set in a live font. It is always the outlined lockup from `assets/brand/vector`.

## Space and shape

- 4 px rhythm (`--s-1` to `--s-9`), with fluid section padding (`--chip-pad`) and gutters (`--gutter`).
- Chips use `--r-chip` (20–36 px). Inner panels use `--r-2` (12 px). Buttons are pills.
- Two-column chips run 5:7. Below 64 rem they stack to one column.

## Components

- **Chip** (`.chip[data-chip=N]`): the section container, with a rivet and a `ChipLabel`.
- **Buttons** (`.btn`, `.btn--ghost`): pills that lift 2 px on hover.
- **Status tags** (`.status[data-status=live|building|planned]`): a filled dot means it exists or is being built, a hollow ring means planned. Every claim about a platform or service carries one.
- **Swatch cards** (services, plans): a rounded panel with a small rivet, the name at display size, and a rule above the action.
- **Readout** (the Ink bar in "Owns every pixel"): mono numbers in tabular figures.

## Motion

- GSAP with ScrollTrigger, loaded on demand, scrubbed to scroll: the deck fans open, the fan folds shut, the Everywhere chip pins and steps through seven hosts, benchmark bars grow, and paint strokes cross out the refused practices.
- The hero mark is a Rive file generated from the brand master (`deno task rive`). If Rive misses its 2.2 s deadline, a CSS stroke paints the wordmark instead.
- The caret blinks in hard steps (about 1 s, no fade), matching the brand rule.
- **Reduced motion:** nothing animates, and GSAP, Rive and the wasm engine are never requested. Every section's static state is its final state, so nothing is missing without motion.

## Performance rules

- Static HTML with inlined CSS. Scripts are small modules per section, and anything heavy (GSAP, Rive, the v1 wasm engine) loads only when it's needed.
- The v1 engine (about 420 KB gzipped) loads only as its section nears the viewport, and never on Save-Data or reduced motion.
- CI runs Lighthouse on every PR (`deno task lighthouse`) and prints the scores in the run summary. Desktop must reach 95+ in every category, LCP under 1.5 s and CLS 0. Mobile (slow 4G, 4× CPU) must reach the scores and CLS 0, and LCP over 1.5 s shows as a warning.
