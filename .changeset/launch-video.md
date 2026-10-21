---
website: none
---

# Add the launch film, rendered with Remotion from real canvist output

- `tools/footage` (`canvist_footage`, unpublished) drives StarterKit editors, `canvist_layout` with `parley` and the bundled Atkinson Hyperlegible faces, and three `canvist_collab` peers over Yjs, and writes every frame as SVG painted by `to_svg`, with keystroke timings measured while recording.
- `video/` is a TypeScript Remotion project that turns that footage into a 65-second film in the Wet Caret motion language: a 16:9 cut at 1080p and 4K, and 9:16 and 1:1 cuts. `npm run render` regenerates the footage, the benchmark numbers (from the budget table in `docs/src/guides/performance.md`), and the typed-API scene's real `tsc` diagnostic, then renders every cut and the poster in `assets/brand/video-poster.png`.
- A `video` CI workflow renders a 540p preview on pull requests that touch the film or the crates it films.
