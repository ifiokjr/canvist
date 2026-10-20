---
website: change
---

# Show every package readme in the docs, without its GitHub header

The docs' Packages section now includes `@canvist/web`, `@canvist/react`, `@canvist/vue`, `@canvist/headless`, and `@canvist/canvas`, whose lowercase `readme.md` files the loader used to miss on case-sensitive file systems. Readmes open with a centred logo, links, and badges for GitHub and the registries; the site drops that whole header, since its own page header already carries the logo.
