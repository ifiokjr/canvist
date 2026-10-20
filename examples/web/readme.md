<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/web.yml"><img src="https://github.com/canvist/canvist/actions/workflows/web.yml/badge.svg?branch=main" alt="web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist web examples

One Vite project with five pages:

| Page             | Shows                                                                                                    |
| ---------------- | -------------------------------------------------------------------------------------------------------- |
| `vanilla.html`   | `createEditor` and a toolbar kept in step with `subscribe`, no framework                                 |
| `react.html`     | `@canvist/react`: `useCanvistEditor` and `useEditorState`, in Strict Mode                                |
| `vue.html`       | `@canvist/vue`: `useCanvistEditor` and `useEditorState` in a single-file component                       |
| `ssr.html`       | HTML from `@canvist/headless/ssr`, rendered on each request, that the editor hydrates without moving     |
| `ssr-react.html` | A React tree rendered on the server with `renderToString` and hydrated with `hydrateRoot`, as in Next.js |

`server.ts` runs Vite in middleware mode and renders the two server-rendered pages on every request, the way Next.js and Nuxt do. The theme, width, and document they share are in `src/ssr-content.ts`.

```sh
npm install
npm run dev     # node server.ts, on http://localhost:5180
npm test        # Playwright, in Chromium, Firefox, and WebKit
```

The packages are linked from this repository, so the build and test steps of `packages/canvist-headless`, `packages/canvist-web`, `packages/canvist-react`, and `packages/canvist-vue` must have run first. Inter is © The Inter Project Authors, under the SIL Open Font License (`public/fonts/OFL.txt`).
