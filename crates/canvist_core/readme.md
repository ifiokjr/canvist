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
  <a href="https://docs.rs/canvist_core"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_core"><img src="https://img.shields.io/crates/v/canvist_core?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_core"><img src="https://img.shields.io/docsrs/canvist_core" alt="docs.rs status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/ci.yml"><img src="https://github.com/canvist/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_core

The platform-agnostic editing engine behind [canvist](https://github.com/canvist/canvist), built on [`canvist_model`](../canvist_model). It has no UI: hosts send it input and draw what it returns.

Most apps add it through [`canvist`](../canvist), the crate to install, as `canvist::core`: `cargo add canvist`.

- **One state:** the document, selection, stored marks, and IME composition change through one path, so they never disagree.
- **Commands:** typing, deleting by grapheme, word, or line, Enter and Backspace that understand lists and quotes, marks, block types, and copy and paste.
- **Selection motion:** by grapheme and word (UAX #29), line, and document. The layout engine answers line and vertical moves.
- **Extensions:** history, keymaps (`Mod` is Cmd on Apple platforms and Ctrl elsewhere), input rules such as `##` to a heading, and paste rules, so an app pays only for what it registers.
- **Typed handles:** registering an extension returns the only typed way to reach its commands and queries. `can()` dry runs and all-or-nothing `chain()`s change nothing unless they apply.

Dynamic hosts call commands by name with checked arguments, and `Editor::metadata_json` describes every command for generated bindings. The StarterKit extensions are in [`canvist_extensions`](../canvist_extensions).
