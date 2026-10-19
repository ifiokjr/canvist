---
"@canvist/web": none
"canvist_layout": test
---

# Test canvas and server-rendered HTML for pixel parity in every browser

- Playwright renders a fixture matrix (system and web fonts, a variable font, CJK with `unicode-range` subsets, Arabic and Hebrew, Devanagari, three emoji formats, letter spacing, sizes from 10 to 64 px) as the canvas, the SSR HTML, and SVG, at device pixel ratios 1 to 3, fractional zoom, and light and dark themes, in Chromium, Firefox, and WebKit on macOS and Linux.
- Line boxes, runs, and caret stops are compared exactly; pixels with pixelmatch and SSIM against per-browser limits, with every known difference documented. Each run uploads a side-by-side report.
- A Rust test holds `parley`, with the same Inter file, to Chromium's line breaks.
