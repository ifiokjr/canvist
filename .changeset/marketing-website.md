---
"canvist": patch
---

Add canvist's website in `website/`: an Astro site with the landing page, preview pricing with a monthly-active-document calculator, the Sync, Convert and History service pages, and a `/docs` section that renders `docs/src` with Pagefind search.

- The landing page is a "fan deck" of Paint swatches that fans open as you scroll. The hero's Wet Caret paints the wordmark in Rive (`website/rive/`, generated from the brand masters). The page also has the v1 engine typing live with a keystroke-to-paint readout, one document morphing across seven hosts, simulated multiplayer carets, a TypeScript error that lights up when an extension is missing, and benchmark bars read from the architecture doc's budget table at build time.
- Every animation has a reduced-motion state, and GSAP, Rive and the wasm engine never load under reduced motion.
- The site deploys to Railway as the `website` service, built from `website/Dockerfile` at the repository root and served by Caddy.
- A new `website` CI workflow runs the type check, unit tests, the build, Playwright (navigation, reduced motion, responsiveness at 320, 768, 1280 and 1920 px, and axe accessibility checks) and Lighthouse CI. A macOS job compares visual snapshots.
