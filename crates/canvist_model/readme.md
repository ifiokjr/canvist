<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img alt="canvist" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" width="220">
    </picture>
  </a>
</p>

# canvist_model

The block-based rich-text document model behind [canvist](https://github.com/ifiokjr/canvist).

- **Schema:** blocks, marks, and inline embeds, contributed by the extensions an app registers.
- **Document:** blocks addressed by stable ids. Text blocks hold one string with run-length formatting.
- **Steps:** invertible edits that map one-to-one onto Yjs operations and report how carets move.
- **Transactions:** apply steps atomically, roll back on failure, and describe exactly which blocks changed.
- **JSON:** reads and writes the ProseMirror/TipTap JSON format.

Applying a keystroke takes about 0.3 µs, whether the document has 100 blocks or 100,000. Run `cargo bench -p canvist_model` to reproduce.
