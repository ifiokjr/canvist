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
| Keystroke with every extension, 100k blocks        | < 5 µs                                      | 0.78 µs              |
| Relayout one edited paragraph, 10k or 100k blocks  | < 100 µs                                    | 2.5 µs               |
| Relayout after a keystroke in a 100 KB paragraph   | < 100 µs                                    | 25 µs                |
| Relayout after Enter, 10k / 100k blocks ¹          | < 200 µs / < 2 ms                           | 83 µs / 1.4 ms       |
| Hit test / caret rectangle                         | < 5 µs / < 2 µs                             | 0.8 µs / 0.01 µs     |
| Display list for one screen                        | < 100 µs                                    | 5 µs                 |
| Scroll one screen into unmeasured blocks           | < 1 ms                                      | 53 µs                |
| Relayout after a web font loads, 10k blocks        | < 200 µs                                    | 43 µs                |
| Measure cache hit rate: resize / typing            | ≥ 99% / ≥ 85%                               | 100% / 98.5%         |
| Measure cache memory per segment, per face         | ≤ 160 B                                     | 144 B                |
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
| Headless TypeScript wasm module, after `wasm-opt`  | < 1,200 KB raw                              | 1,100 KB (451 KB gz) |
| Headless Dart: one command / Yjs update            | < 10 µs / < 30 µs                           | 2.0 µs / 3.5 µs      |
| Keystroke → paint on the web, p95 ⁴                | < 8 ms                                      | 0.5 / 2 / 2 ms       |
| Typing burst: 1,000 key events on the web ⁴        | no crash, no dropped input, ≤ 2 frames late | 1,000 of 1,000       |
| Open a 10k-paragraph document → first paint        | < 50 ms                                     | layout 0.6 ms        |
| Web bundle (core + StarterKit), gzipped            | < 350 KB                                    | 349.1 KB ³           |

¹ Pressing Enter grows slowly with document size, about 10 ns per block, because every later line moves down one row. It stays well under a frame at 100,000 blocks.

² Structural edits, such as splitting or joining blocks, cost time in proportion to the number of blocks beside them, because Yjs walks the list of siblings. Budgets use 10,000 top-level blocks.

³ Measured as an app ships it: a Vite build of a page that creates an editor, minified and gzipped (335 KB of WebAssembly, 14 KB of script). CI fails above the budget. The module is optimized without inlining, which gzips smaller even though it is larger raw. Line breaking's Unicode data is 13.6 KB of it, and browsers offer no line-break segmenter to use instead.

⁴ Chromium / Firefox / WebKit on an Apple silicon Mac, measured by `editor.inputLatency()` in canvist's Playwright tests: from each input event's timestamp until the change is painted on the canvas, in the same task. Firefox and WebKit round timestamps to 1 ms. In the burst, every key is painted in its own task, so none waits for a later frame; the 95th percentile is 0.4 / 3 / 1 ms. The browser's Event Timing API also counts the wait for the next frame, and reports only events that took 16 ms or more, so it can't resolve an 8 ms budget. The tests record its slow entries beside these numbers.

## Regression checks on every pull request

The budgets above are absolute: an operation fails when it takes longer than its limit, however long it took before. Two more checks run on every pull request and compare it with the commit it merges into. Both are required, so a pull request can't merge while either fails.

### `bench / speed`

The check runs every divan benchmark in the crates the pull request can change (model, editor keystrokes, extensions, layout, collaboration, headless, scene, canvas, and paint) on the base and on the head, on one runner, in one job. The base is the commit the pull request merges into, so the comparison shows exactly what the pull request changes.

Each bench binary runs five times per side, alternating base and head so that a runner warming up or slowing down affects both. Every run gives one median per benchmark, and a time in the report is the median of those medians. A benchmark **regresses** only when both of these hold:

- the head is slower than the base by more than the benchmark's threshold, and
- every head run is slower than every base run.

The second rule is what keeps runner noise from failing a pull request. A busy neighbour on the runner slows one run, not all of one side and none of the other. When a benchmark looks regressed, its suite runs five more times per side, and the verdict uses all ten runs.

The report, in the job summary and in a comment on the pull request, lists what regressed or improved first, then every benchmark:

| Column    | Meaning                                                               |
| --------- | --------------------------------------------------------------------- |
| Base      | the median time on the commit the pull request merges into            |
| Head      | the median time with the pull request                                 |
| Change    | head over base: `+12.0%` is 12% slower                                |
| Threshold | the slowdown this benchmark may show before it fails                  |
| Verdict   | ❌ regressed, 🚀 improved, `new` or `removed`, or empty for no change |

Suites the pull request can't affect don't run, and the report names them. A suite is affected when a file changes in its crate or in any workspace crate it depends on, or when a global input changes: `Cargo.lock`, `Cargo.toml`, `rust-toolchain.toml`, `.cargo`, `tools/bench`, or the workflow. Every push to `main` runs every suite and caches its bench binaries, so most pull requests only build the head.

Thresholds live in [`tools/bench/speed.json`](https://github.com/canvist/canvist/blob/main/tools/bench/speed.json): `threshold` is the default, in percent, and `thresholds` sets one benchmark by its id (`model/type_char/100000`) or a group by a pattern (`scene/to_json/*`). The longest matching pattern wins. Raise a threshold only for a benchmark that is noisy on the runner, never to let a slowdown through. A slowdown you mean to make, such as trading speed for correctness, should say so in the pull request, and the new speed becomes the base once it merges. Download the `bench-speed-samples` artifact to see every run's numbers.

### `bench / bundle size`

The check builds every shipped artifact and measures it raw, gzipped, and brotli-compressed:

- `@canvist/web`: the app bundle (a Vite build of a page that creates an editor), its WebAssembly, and its JavaScript;
- `@canvist/headless`: the document and scene WebAssembly modules and the JavaScript;
- `@canvist/react` and `@canvist/vue`;
- `@canvist/canvas`: its WebAssembly and JavaScript;
- `libcanvist_ffi` built for Linux.

An artifact **fails** when it is over its budget, or when its gzipped size grew by more than both 1% and 1,024 bytes since the base. Growth you mean to ship passes once the pull request has the `size-increase-approved` label, and adding the label re-runs the check. The label never waives a budget: raise the budget in [`tools/bench/size.json`](https://github.com/canvist/canvist/blob/main/tools/bench/size.json) in the same pull request, so the change is reviewed.

Builds are deterministic, so every push to `main` records its sizes and pull requests compare with that record. An artifact the pull request can't affect isn't rebuilt; the report marks it `(unchanged)` and still checks it against its budget.

The iOS and Android libraries (`CanvistFFI.xcframework`, `libcanvist_ffi.a`, and `libcanvist_ffi.so` for each Android ABI) only build in the `native` workflow, which runs on pull requests that touch the native surface. It checks them against their budgets in `size.json` and reports their sizes in its job summary.

### Run them yourself

Compare your branch with `main` the way CI does:

```sh
git worktree add ../canvist-base main
ln -s "$PWD/target" ../canvist-base/target
node tools/bench/speed.ts build --root ../canvist-base --suites "model keystroke" --out .bench/base
node tools/bench/speed.ts build --root . --suites "model keystroke" --out .bench/head
node tools/bench/speed.ts compare --base .bench/base --head .bench/head --suites "model keystroke"
```

Measure sizes the same way, with `node tools/bench/size.ts build --root . --groups web`, then `measure` and `compare`. The commands are documented at the top of each script.

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
