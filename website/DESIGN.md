# canvist website design

Recorded from the built site (October 2026). [PRODUCT.md](./PRODUCT.md) holds the product facts; this file holds the visual system. Tokens live in [`src/styles/tokens.css`](./src/styles/tokens.css) and the chip system in [`src/styles/global.css`](./src/styles/global.css).

## The world: a paint fan deck

The site is a fan deck of canvist's own colour, laid out calmly. The landing page has six beats, each with one idea, one visual and one or two sentences: the hero, the live editor, one document on every platform, carets writing together, free forever plus the services, and the call to action. Beats alternate between swatch chips (a rounded colour field punched with a rivet in its top-left corner) and open sections on the white page, so the chips stay special.

The hero shows the rest of the deck fanned behind the film poster as plain colour. Detail lives elsewhere: benchmarks in the architecture doc, the platform matrix in the roadmap, API samples in the docs, and service detail on the service pages.

## Colour

- **Brand:** Ink `#17141F`, Paint `#3B2CF5`, Paper `#F5F3FF`, Paint on dark `#8A80FF` (from `assets/brand/readme.md`).
- **The ramp:** `--chip-0` to `--chip-7` step from Paper through Paint tints to Ink in light mode, and lift from Ink to pale violet in dark mode. `--chip-4` is Paint in both themes. Each section takes the next chip, so the page darkens (or lightens) as it goes, like paint drying.
- **Text on chips:** `--chip-fg`, and `--chip-muted` (86% of the foreground mixed into the chip) for secondary text. That is the most a muted tone can fade and still pass WCAG AA on every chip; don't go lower.
- **The caret:** only Paint, Paint on dark, or one mono colour (Paper on a Paint chip), per the brand rules.
- **Semantic exceptions:** `--error` for type errors and over-budget values. The four presence colours in the Together demo (`#3B2CF5`, `#E5484D`, `#0E9384`, `#C07A00`) are the only other hues.
- **Themes:** one `light-dark()` declaration per token, driven by `color-scheme`. `data-theme` on `<html>` pins a theme and the toggle stores it.

## Type

- **Bricolage Grotesque** (variable, optical size and weight) for everything that isn't code. Display uses weights 750–800 with tight tracking (`-0.035em` to `-0.05em`); body copy uses the text optical size at 17 px with a 1.55 line height.
- **Geist Mono** only for measurements, readouts and code. No eyebrows or section numbers.
- Both are subset to the characters the copy uses (`deno task fonts`) and load with `font-display: optional`, so they never shift layout.
- Scale: `--t-mega` (hero numerals, closing titles), `--t-1` (section titles), `--t-2`, `--t-3` (ledes), body, `--t-small`, `--t-micro`.
- The wordmark is never set in a live font. It is always the outlined lockup from `assets/brand/vector`.

## Space and shape

- 4 px rhythm (`--s-1` to `--s-9`), with fluid section padding (`--chip-pad`) and gutters (`--gutter`).
- Chips use `--r-chip` (20–36 px). Inner panels use `--r-2` (12 px). Buttons are pills.
- Two-column beats run 5:7 (or 7:5 to alternate). Below 64 rem they stack to one column.

## Components

- **Chip** (`.chip[data-chip=N]`): a content-sized colour field with a rivet. No label strips, no clipping, no viewport heights.
- **Beat** (`.beat`): an open section on the page, between chips.
- **Buttons** (`.btn`, `.btn--ghost`): pills that lift 2 px on hover.
- **Status tags** (`.status[data-status=live|building|planned]`): a filled dot means it exists or is being built, a hollow ring means planned. Every claim about a platform or service carries one.
- **Swatch cards** (plans on the pricing page): a rounded panel with a small rivet. On the landing page the services are a ruled list, not cards.
- **Readout** (under the live editor): mono numbers in tabular figures.

## Motion

- **One authored moment:** the caret painting the wordmark. The hero mark is a Rive file generated from the brand master (`deno task rive`). If Rive misses its 2.2 s deadline, a CSS stroke paints the wordmark instead. The caret blinks in hard steps (about 1 s, no fade).
- **One easing:** `--ease-wet`, `cubic-bezier(0.16, 1, 0.3, 1)`: a fast start and a long, wet settle.
- **Only `transform` and `opacity` animate.** There are no scroll-linked scripts, no pinned sections and no GSAP.
  - Chips below the fold settle once as they scroll in. They start 2.5 rem low and transparent, and a CSS transition brings them home.
  - The platform frames crossfade.
  - The fan opens once on load.
- **Loops run only on screen:** the caret demos run only while visible. The live editor starts only when its section is on screen and scrolling has paused, so its start-up never lands on a scrolling frame.
- **Reduced motion:** nothing animates, Rive is never requested, and the editor loads only when the visitor asks for it. Every section's resting state is its final state, so nothing is missing without motion.

## Performance rules

- Static HTML with inlined CSS. Scripts are small modules per section, and anything heavy (Rive, the wasm editor) loads only when it's needed.
- The editor (`@canvist/web`, under 350 KB gzipped) loads only when its section is on screen and scrolling has paused. Under Save-Data or reduced motion it loads only when the visitor presses the document or its button.
- Scrolling the landing page stays inside one 60 Hz frame with no long tasks, on desktop and with the CPU throttled 4× (`deno task perf:scroll`, reported in CI).
- Nothing may be cut off at any common screen size. A Playwright check scrolls every page across nine viewports, from 360 × 640 to 2560 × 1440 including landscape phones. It fails on clipped content or horizontal overflow.
- CI runs Lighthouse on every PR (`deno task lighthouse`) and prints the scores in the run summary. Desktop must reach 95+ in every category, LCP under 1.5 s and CLS 0. Mobile (slow 4G, 4× CPU) must reach the scores and CLS 0, and LCP over 1.5 s shows as a warning.
