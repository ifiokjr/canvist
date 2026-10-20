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
  <a href="https://docs.rs/canvist_extensions"><strong>API</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_extensions"><img src="https://img.shields.io/crates/v/canvist_extensions?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_extensions"><img src="https://img.shields.io/docsrs/canvist_extensions" alt="docs.rs status" /></a>
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/ci.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_extensions

The [canvist](https://github.com/ifiokjr/canvist) StarterKit: the extensions TipTap's StarterKit ships, built on [`canvist_editor`](../canvist_editor) and named the way TipTap names them, so documents move between the two as ProseMirror JSON.

- **Nodes:** paragraph, heading (levels 1 to 6), blockquote, code block, hard break, horizontal rule, bullet list, ordered list, list item, and task list.
- **Marks:** bold, italic, underline, strike, code, and link (non-inclusive, with autolinking and linking on paste).
- **Behaviour:** Markdown-style input rules (`##`, `-`, `1.`, `[ ]`, `>`, `` ``` ``, `---`, `**bold**`), TipTap's key bindings, undo and redo keys, and a trailing paragraph.

Each extension is behind its own Cargo feature, and `starter-kit` (the default) enables them all, so an app compiles only what it registers.

```rust
use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_extensions::HeadingLevel;
use canvist_extensions::StarterKit;

let mut builder = Editor::builder();
let kit = StarterKit::add_all(&mut builder);
let mut editor = builder.build()?;
let env = Env::headless(std::time::Duration::ZERO);

// Typed commands, dry runs, and chains: no UI, nothing magic.
if editor.can(&kit.bold.toggle(), &env)? {
	editor.chain().then(kit.bold.toggle()).then(kit.heading.set(HeadingLevel::H2)).run(&env)?;
}
let level = kit.heading.level(editor.state());
```

Each handle exists only if its extension was registered, so code calling an extension the app left out does not compile. Dynamic hosts use the same commands by name (`editor.run_named("toggleHeading", &Attrs::new().with("level", 2), &env)`), and `editor.metadata_json()` describes every command, query, and attribute for generated bindings.

Typing a keystroke through the whole kit (keymaps, input rules, history, and fix-ups) costs about 0.6 µs at 100 to 100,000 blocks; a space, which runs every input rule, about 3.5 µs. Run `cargo bench -p canvist_extensions` to reproduce.
