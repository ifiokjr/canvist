<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/quick-start/server-rust.md"><strong>Quick start</strong></a> ·
  <a href="https://docs.rs/canvist_model"><strong>API</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_model"><img src="https://img.shields.io/crates/v/canvist_model?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_model"><img src="https://img.shields.io/docsrs/canvist_model" alt="docs.rs status" /></a>
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/ci.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_model

The block-based rich-text document model behind [canvist](https://github.com/ifiokjr/canvist).

- **Schema:** blocks, marks, and inline embeds, contributed by the extensions an app registers.
- **Document:** blocks addressed by stable ids. Text blocks hold one string with run-length formatting.
- **Steps:** invertible edits that map one-to-one onto Yjs operations and report how carets move.
- **Transactions:** apply steps atomically, roll back on failure, and describe exactly which blocks changed.
- **JSON:** reads and writes the ProseMirror/TipTap JSON format.

Applying a keystroke takes about 0.3 µs, whether the document has 100 blocks or 100,000. Run `cargo bench -p canvist_model` to reproduce.
