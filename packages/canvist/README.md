<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/playwright.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/playwright.yml/badge.svg?branch=main" alt="playwright status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/canvist (v1)

The first canvist editor for the browser, which still powers the live demo on the website. It isn't published, and [`@canvist/web`](../canvist-web) replaces it: the v2 editor, typed from the extensions you register, with server-rendered HTML it hydrates without moving.

<details>
<summary><strong>Development</strong></summary>

```bash
deno task build:wasm        # build the WebAssembly module
deno task test:unit         # unit tests
deno task test:playwright   # browser tests
```

The demos are `demo/index.html` and `demo/collab.html`, which syncs two tabs through `BroadcastChannel`.

</details>
