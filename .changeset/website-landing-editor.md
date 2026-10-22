---
website: feat
---

# Embed the web editor on the landing page

The "Type into the real engine" demo now runs `@canvist/web`, the editor canvist ships.

- The page arrives with the demo's document as HTML, rendered at build time by `@canvist/headless/ssr` with the page's own Bricolage and Geist Mono faces. The editor hydrates from it and swaps in the canvas in one frame, with no layout shift. Visitors without JavaScript, and crawlers, see the text.
- The editor still loads only once the section is on screen and scrolling has settled. Under reduced motion or Save-Data it loads when the visitor presses the document or the button. Starting it leaves focus and the scroll position where they were.
- The sheet follows the site's light and dark themes, and the theme toggle recreates the editor in the other scheme, keeping the document, selection, and focus.
- The readout's values have room for "123.4 ms", so new numbers no longer move the labels.
- The website build compiles `@canvist/headless` and `@canvist/web` from the workspace, in a `packages` CI job and a Rust stage in `website/Dockerfile`.
