# Performance

Read this page to see what canvist costs: the time each operation takes, how that grows with document size, and how to keep your app fast.

## The short version

- **Typing costs the same in a 10-page document and a 10,000-page one.** A keystroke changes one block, and only that block is laid out again.
- **A command costs under a microsecond in Rust**, and a few microseconds from TypeScript or Dart.
- **Loading or exporting 10,000 paragraphs takes milliseconds.**
- **Collaboration is cheap.** Applying a collaborator's keystroke costs a few microseconds.

## Performance budgets

Every number below has a budget, and a test fails the build when an operation goes over it. "Today" is measured on Apple silicon. Rows marked "Coming soon" belong to editor hosts that haven't shipped yet.

| Metric                                             | Budget                                      | Today                |
| -------------------------------------------------- | ------------------------------------------- | -------------------- |
| Apply one keystroke (model), 100k blocks           | < 2 µs                                      | 0.29 µs              |
| Enter / Backspace across blocks, 100k blocks       | < 100 µs                                    | 45 µs                |
| Keystroke in a 100 KB paragraph                    | < 5 µs                                      | 0.94 µs              |
| Relayout one edited paragraph, 10k or 100k blocks  | < 100 µs                                    | 2.5 µs               |
| Relayout after a keystroke in a 100 KB paragraph   | < 100 µs                                    | 25 µs                |
| Relayout after Enter, 10k / 100k blocks ¹          | < 200 µs / < 2 ms                           | 83 µs / 1.4 ms       |
| Hit test / caret rectangle                         | < 5 µs / < 2 µs                             | 0.8 µs / 0.01 µs     |
| Display list for one screen                        | < 100 µs                                    | 5 µs                 |
| Scroll one screen into unmeasured blocks           | < 1 ms                                      | 53 µs                |
| Mirror a keystroke into a Yjs update, 100k blocks  | < 20 µs                                     | 1.9 µs               |
| Apply a collaborator's keystroke, 100k blocks      | < 20 µs                                     | 2.7 µs               |
| Mirror Enter into Yjs, 10k blocks²                 | < 150 µs                                    | 45 µs                |
| Apply a collaborator's Enter, 10k blocks²          | < 400 µs                                    | 114 µs               |
| Join a 10k-paragraph document from Yjs state       | < 50 ms                                     | 24 ms                |
| Headless: load 10k paragraphs (Markdown / JSON)    | < 50 ms / < 50 ms                           | 10 ms / 17 ms        |
| Headless: export 10k paragraphs (JSON / MD / HTML) | < 50 / 25 / 25 ms                           | 19 / 7 / 2.4 ms      |
| Headless: run one command, 10k paragraphs          | < 2 µs                                      | 0.21 µs              |
| Headless: create a StarterKit document             | < 250 µs                                    | 56 µs                |
| Headless: apply one Yjs keystroke, 10k paragraphs  | < 20 µs                                     | 2.3 µs               |
| Headless TypeScript: one command / Yjs update      | < 5 µs / < 30 µs                            | 0.71 µs / 4.6 µs     |
| Headless TypeScript wasm module, after `wasm-opt`  | < 1,200 KB raw                              | 1,058 KB (433 KB gz) |
| Headless Dart: one command / Yjs update            | < 10 µs / < 30 µs                           | 2.0 µs / 3.5 µs      |
| Keystroke → paint on the web (Event Timing), p95   | < 8 ms                                      | Coming soon          |
| Typing burst: 1,000 key events in one task         | no crash, no dropped input, ≤ 2 frames late | model passes 20k ops |
| Open a 10k-paragraph document → first paint        | < 50 ms                                     | layout 0.6 ms        |
| Web bundle (core + StarterKit), gzipped            | < 350 KB                                    | Coming soon          |

¹ Pressing Enter grows slowly with document size, about 10 ns per block, because every later line moves down one row. It stays well under a frame at 100,000 blocks.

² Structural edits, such as splitting or joining blocks, cost time in proportion to the number of blocks beside them, because Yjs walks the list of siblings. Budgets use 10,000 top-level blocks.

## Keep your app fast

- **Build one engine and share it.** Creating an engine registers its extensions. A document from an existing engine takes about 60 µs.
- **Reuse a `Renderer` per thread.** It caches text measurements, so the second render of similar text is much cheaper.
- **Store JSON.** It keeps block ids, so collaborators and comments don't have to re-anchor after a reload.
- **Mind the WebAssembly size.** The TypeScript package's module is 433 KB gzipped. Collaboration is most of it: the core without it is 196 KB.
- **Batch updates on the server.** `takeUpdates()` returns everything since the last call, so you can send one message per tick instead of one per keystroke.

## Measure it yourself

Every number in the table comes from a benchmark in the repository:

```sh
cargo bench -p canvist_headless --all-features
```
