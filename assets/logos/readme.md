# canvist logo alternatives

Four candidate directions for the canvist logo. None of them is final; the owner picks one. A side-by-side comparison (favicon sizes, app icons, lockups on light and dark, a mock editor header, and a recommendation) is in the [comparison artifact](https://claude.ai/artifact/Gr2RqVoGgDZ4pSmgEYWJGU).

| Direction                   | Idea                              | Rationale                                                                                                                                        |
| --------------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| [Wet Caret](./wet-caret/)   | The caret as a brush stroke       | canvist paints every glyph itself, so its text cursor is a stroke of paint that hasn't dried yet: a drip runs off its foot.                      |
| [Loose Tile](./loose-tile/) | Stacked blocks forming the letter | A c set from seven blocks with one lifted mid-drag, because every Enter makes a block you can pick up and move.                                  |
| [Selection](./selection/)   | Highlight and handles             | Three selected lines and two drag handles, the gesture everyone makes in an editor. The lockup selects "ist", the half of the name about craft.  |
| [Riso](./riso/)             | Collaborators' colours converging | Three collaborators print the same c in overlapping risograph inks; overlaps mix instead of overwriting, as the CRDT does with concurrent edits. |

## Files

Each direction folder contains:

- `mark.svg`: the square mark for light backgrounds (64 × 64 view box, legible from 16 px favicons to 512 px icons).
- `mark-dark.svg`: the mark for dark backgrounds.
- `lockup.svg` and `lockup-dark.svg`: the mark with the "canvist" wordmark. Wet Caret's lockup is the word followed by its caret, since the mark is "c" plus the caret.

The marks are hand-built geometry. The wordmarks are outlined to paths, so they render identically without any font installed. They come from these SIL Open Font License faces:

| Direction  | Wordmark face           | Colours                                                                  |
| ---------- | ----------------------- | ------------------------------------------------------------------------ |
| Wet Caret  | Bricolage Grotesque 800 | ink `#17141F`, paint `#3B2CF5` (dark: `#F5F3FF`, `#8A80FF`)              |
| Loose Tile | Unbounded 600           | ink `#1A1D29`, lifted tile `#FF6B2C` (dark: `#ECEEF5`, `#FF7A3D`)        |
| Selection  | Instrument Sans 600     | highlight `#FFC53D`, ink `#16161D` (dark handles: `#F6F4EE`)             |
| Riso       | Syne 800                | riso pink `#FF48B0`, blue `#0078BF`, sunflower `#FFB511`, mixed overlaps |

Riso's overlaps are drawn as solid clipped fills (multiply mix on light, screen mix on dark) rather than blend modes, so favicons and every SVG renderer show the same colours.
