---
"canvist": patch
---

Redesign the website's landing page for clarity and smooth motion.

- The landing page has six beats with one idea each: the hero, the live editor, one document on every platform, writing together, free forever plus the services, and the call to action. Benchmarks, the platform matrix and API samples now live in the docs.
- Motion is CSS transitions on `transform` and `opacity` only, with one easing. Nothing is pinned or scroll-linked, and GSAP is gone. Scrolling stays inside one 60 Hz frame with no long tasks, even with the CPU throttled 4×.
- Nothing is cut off at any common screen size. A new Playwright check scrolls every page across nine viewports and fails on clipped content or horizontal overflow. Visual snapshots cover each viewport's first screen.
- The live v1 editor no longer steals keyboard focus when it loads, and loads only while its section is on screen and scrolling has paused.
