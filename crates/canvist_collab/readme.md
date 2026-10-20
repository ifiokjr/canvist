<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/server-rust.md"><strong>Quick start</strong></a> ·
  <a href="https://docs.rs/canvist_collab"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_collab"><img src="https://img.shields.io/crates/v/canvist_collab?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_collab"><img src="https://img.shields.io/docsrs/canvist_collab" alt="docs.rs status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/ci.yml"><img src="https://github.com/canvist/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_collab

Real-time collaboration for [canvist](https://github.com/canvist/canvist) documents over Yjs, using [`yrs`](https://crates.io/crates/yrs).

- **Binding:** mirrors a `canvist_model` document into a yrs document and back. Each model step becomes yrs operations and each remote yrs event becomes steps, so nothing is ever diffed.
- **Plain Yjs:** the shared document follows a documented schema of `Y.Map` blocks, `Y.Text` with marks as formatting, and nested `Y.Array`s, so JavaScript Yjs peers, y-websocket, and Hocuspocus take part without knowing about canvist.
- **Robust:** updates may arrive out of order or twice, blocks that break the schema are hidden rather than deleted, and concurrent moves of one block keep a single copy.
- **Shared undo:** a yrs `UndoManager` that only undoes local changes, behind a `History` trait the editor can swap.
- **Sync and presence:** a transport-agnostic y-sync session for y-websocket and Hocuspocus servers, and awareness carets that stay on their characters while others type.

Mirroring a keystroke into a Yjs update takes about 2 µs, and applying a remote keystroke about 2.5 µs, whether the document has 100 blocks or 10,000. Run `cargo bench -p canvist_collab` to reproduce.

The compatibility tests run a real JavaScript Yjs peer under [Deno](https://deno.com), which the devenv shell provides.
