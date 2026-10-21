<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/desktop.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/desktop.yml"><img src="https://github.com/canvist/canvist/actions/workflows/desktop.yml/badge.svg?branch=main" alt="desktop status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist notes

A small Markdown editor on [`canvist_desktop`](../../crates/canvist_desktop), for macOS, Windows, and Linux.

- **Toolbar:** headings, bold, italic, strike, code, lists, tasks, quotes, code blocks, undo, and redo. Buttons run the editor's typed StarterKit commands and light up when their format is active at the caret.
- **Open and save Markdown** with the system's file dialogs, or Cmd-O and Cmd-S (Ctrl elsewhere). Shift-Cmd-S saves as.
- **Live collaboration** with `--collab`: the document is a y-websocket room's, shared with every other client in it.

```sh
cargo run -p canvist_desktop_example -- notes.md
```

To collaborate, start a y-websocket server, then open the same room in two windows:

```sh
npx y-websocket
cargo run -p canvist_desktop_example -- --collab ws://localhost:1234/notes
```

The toolbar is drawn into the same vello scene as the editor, with parley for its labels; `EditorWindow` places the editor below it and routes the window's events.
