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
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/ci.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_cloud

The server behind canvist Cloud, and the exact binary self-hosters run. It is not published to crates.io; build it from this repository or run the container image.

- **Sync:** a multi-tenant y-sync relay. Stock [`y-websocket`](https://github.com/yjs/y-websocket) and [Hocuspocus](https://tiptap.dev/docs/hocuspocus) clients connect without changes, with presence, read-only sessions, and per-document access from short-lived tokens.
- **Durable:** documents persist to Postgres as Yjs updates and are compacted into snapshots; idle documents leave memory and reload on demand. A graceful shutdown persists everything and tells clients to reconnect.
- **Webhooks:** a debounced, signed `document.updated` webhook per environment.
- **REST:** server-side keys mint client tokens, list, read, write, and delete documents.

Measured on Apple silicon (`test:perf`): one keystroke reaches the last of 100 connected peers in 0.6 ms; a loaded 50-paragraph document holds about 130 KiB; an idle connection about 17 KiB.

See [the API reference](../../docs/src/cloud/api.md) and [self-hosting](../../docs/src/cloud/self-hosting.md).

```sh
CANVIST_TOKEN_SECRET=$(openssl rand -hex 32) cargo run -p canvist_cloud
```

The integration tests drive real JavaScript clients under [Deno](https://deno.com), which the devenv shell provides. Set `CANVIST_TEST_DATABASE_URL` to run them against Postgres instead of memory.
