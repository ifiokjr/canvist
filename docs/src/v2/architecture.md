# canvist v2 architecture

canvist is a rich-text editor that owns every pixel. One Rust core holds the document, the editing logic, and text layout. Thin hosts on each platform turn native input into editor commands and paint a display list with the platform's own drawing API. This page records the decisions behind v2 and why they were made. [The roadmap](./roadmap.md) splits the work into milestones, and [the research brief](../research/editor-landscape.md) holds the evidence.

## Goals

1. **Speed you can measure.** The time from a keystroke to the painted character is a tracked number with a budget, benchmarked on every change.
2. **Feature parity with TipTap v3**, built from opt-in extensions so an app pays only for what it registers.
3. **Collaboration as a first-class feature**, built on `yrs` and wire-compatible with Yjs peers and servers (y-websocket, Hocuspocus).
4. **Every platform:** the web (vanilla DOM, React, Vue, SSR), native desktop (macOS, Windows, Linux), native iOS and Android, Flutter, React Native, and Lynx.
5. **No jank on handoff.** A server-rendered HTML or SVG representation is replaced by the live canvas with no layout shift, and an end-to-end test checks it.

## Layers

```text
┌───────────────────────────────────────────────────────────────────────────┐
│ Hosts (one per platform)                                                  │
│ web (wasm + Canvas2D + EditContext) · desktop (winit + vello + AccessKit) │
│ iOS (UITextInput + CoreText) · Android (InputConnection) · Flutter · RN   │
└──────────────┬──────────────────────────────────────▲─────────────────────┘
               │ input events, IME, pointer, clipboard │ display list, a11y tree
┌──────────────▼──────────────────────────────────────┴─────────────────────┐
│ canvist_editor: commands, selection motion, keymaps, input rules,         │
│ history, extensions (schema + behaviour), IME composition state           │
├───────────────────────────────────┬───────────────────────────────────────┤
│ canvist_layout                    │ canvist_collab (optional)             │
│ TextMeasurer, line breaking,      │ yrs binding: Step ⇄ Y.Text / Y.Array  │
│ bidi, per-block layout cache,     │ awareness, remote carets,             │
│ virtualization, hit testing,      │ shared undo manager                   │
│ display list, SVG/HTML renderers  │                                       │
├───────────────────────────────────┴───────────────────────────────────────┤
│ canvist_model: Schema, Document, InlineText, Step, Transaction            │
└───────────────────────────────────────────────────────────────────────────┘
```

Each layer depends only on the layers below it. `canvist_model` has three dependencies: `rustc-hash`, `serde_json`, and `thiserror`.

## Decision 1: blocks with stable ids, inline text as mark runs

**What:** a `Document` is an arena of `Block`s addressed by `BlockId`. A block is one of three things:

- a **text block**, holding `InlineText`: one UTF-8 string plus run-length `MarkSet` formatting;
- a **container**, holding an ordered list of child ids;
- a **leaf**, holding nothing.

Inline atoms such as mentions, inline images, and hard breaks are embeds. Each one occupies one object replacement character (U+FFFC) in the string.

**Why:**

- **Typing touches one block.** Inserting a character costs `O(runs in that block)` whatever the document's size. The benchmark shows about 280 ns at 100 to 100,000 blocks.
- **Positions don't move.** A position is `Point { block, offset }`. ProseMirror uses one integer position for the whole document, so every edit shifts everything after it. Here an edit in one block never moves positions in another. Remote carets, comments, and decorations stay put without remapping.
- **The shape matches the CRDT.** Block ids map to `Y.Map`s and inline text maps to `Y.Text` with format attributes, as in Quill/Yjs. Containers map to `Y.Array`s.
- **Layout can be cached.** Each block carries a `rev` counter that layout caches key on, so only blocks whose `rev` changed are laid out again.

**Rejected alternatives:**

- **A ProseMirror-style inline node tree.** It costs more memory and gains nothing for rich text.
- **A flat text model** (canvist v1). It can't represent lists, tables, or drag-and-drop of blocks.

## Decision 2: invertible steps that map one-to-one to CRDT operations

**What:** steps are the only way to change a document:

- inline steps: `InsertInline`, `RemoveInline`, `AddMark`, `RemoveMark`, `RestoreMarks`;
- block steps: `SetBlock`, `Split`, `Join`, `InsertBlocks`, `RemoveBlocks`, `MoveBlocks`.

Applying a step returns:

- its inverse, so undo needs no snapshots;
- a `StepMap` saying how points move;
- the set of touched blocks, which drives layout invalidation.

A `Transaction` applies steps atomically and can roll back. If any step fails, the document is left exactly as it was, and a property test checks this.

**Why:** every step corresponds to one `yrs` operation, or a short fixed sequence of them. So the collaboration binding translates in both directions without diffing. The research found that diffing is the root cause of y-prosemirror's problems: whole-document replaces, lost positions, and deleted nodes on schema conflicts.

**The tradeoff we accepted:**

- **The research brief recommends making the Yjs document itself the source of truth.** We keep a plain Rust mirror instead, for three reasons:
  - Layout, painting, and hit testing read the document thousands of times per frame, and plain struct reads are far cheaper than yrs reads.
  - Apps that don't collaborate shouldn't pay for yrs in wasm size or startup time. M3 measures that cost.
  - The 1:1 step mapping makes the mirror a translation, not a reconciliation.
- **This is a known risk.** If the binding turns out to need diffing anywhere, revisit the decision.
- **`Split` loses concurrent edits.** If a peer edits the tail of a block while another peer splits it, the tail is re-created in a new `Y.Text` and the edit lands on the deleted original. M4 measured it (`canvist_collab/tests/concurrent_split.rs`): inserted text survives but moves to the end of the first block, while deletions and formatting in the tail are lost. Edits before the split point are unaffected. Every Yjs editor shares this limitation.
- **Moving a block deletes and copies it.** yrs 0.28 has no move, so text typed concurrently into a moved block is lost, and when two peers move the same block at once every peer keeps the copy with the smallest yrs item id and deletes the others.

## Decision 3: UTF-8 offsets inside, UTF-16 at the edges

The model uses UTF-8 byte offsets, validated to fall on character boundaries, because Rust strings are UTF-8 and slicing them is free. JavaScript hosts and the `yrs` binding convert offsets at the boundary. The binding sets `OffsetKind::Utf16`: the setting only changes how yrs reads indices locally (the wire format always counts UTF-16), but yrs's sticky indexes, which awareness carets use, mix units under `OffsetKind::Bytes`. Converting within one block costs `O(block length)`. Grapheme-cluster rules for caret movement live in `canvist_editor`, not the model.

## Decision 4: the host measures text, Rust lays it out

Line breaking, bidi, block layout, virtualization, hit testing, and caret geometry run in Rust (`canvist_layout`). Measuring glyph advances goes through a `TextMeasurer` trait, with one implementation per platform:

| Platform        | Measurer                           | Why                                                                                                                                                                                                                                                                                       |
| --------------- | ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Web             | Browser (`measureText`, cached)    | Canvas has no draw-glyphs-by-id call, so text is painted with `fillText`, and the browser's own shaping has to agree with it. Measuring with the browser also makes the server-rendered HTML wrap at exactly the same points as the canvas, which is what makes the handoff free of jank. |
| Desktop and SSR | `parley` (harfrust + skrifa)       | Fully deterministic and needs no system font server. Used for snapshot tests and SVG output.                                                                                                                                                                                              |
| iOS             | CoreText                           | Matches system fonts and Dynamic Type.                                                                                                                                                                                                                                                    |
| Android         | `Paint` / `TextRunShaper`          | Matches system fonts.                                                                                                                                                                                                                                                                     |
| Flutter         | **Open question, prototype first** | `dart:ui` has no draw-glyphs call. The options are measuring with `Paragraph` through batched FFI, or a texture rendered in Rust. This is the largest open risk.                                                                                                                          |

Measurements are cached per (font, text segment). One relayout needs at most one round trip to the host, never a call per word.

How M2 implements it:

- **The measuring contract.** Layout splits text into segments: a word plus its trailing spaces, cut wherever the font or bidi level changes. A host returns one advance per character for each segment, and the advances must sum to the width it paints the whole segment at. Layout reads those sums only at grapheme boundaries, so a host may split a ligature's width among its characters however it likes.
- **Line breaking uses ICU4X (`icu_segmenter` 2.3), not `unicode-linebreak`,** for four reasons:
  - ICU4X's options are defined as CSS's `line-break` and `word-break` values, so the HTML contract can name the same behaviour (`line-break: strict`).
  - Firefox now breaks lines with ICU4X, and Chromium and WebKit use ICU, so the defaults agree with every engine apart from small tailorings. A `BreakOverride` hook covers those tailorings.
  - It is maintained and tracks current Unicode. `unicode-linebreak` has not been released since 2023.
  - `parley` already depends on it, so desktop builds carry one copy of the line-breaking data, not two.
- **What it costs.** Line and grapheme data add about 23 KB gzipped to wasm. The dictionary and LSTM models for Thai, Lao, Khmer, and Burmese are left out.
- **The measured size of each piece in wasm,** after `wasm-opt -Oz`, gzipped:

  | Build                       | Raw    | Gzipped |
  | --------------------------- | ------ | ------- |
  | Model only                  | 85 KB  | 38 KB   |
  | + layout and both renderers | 296 KB | 133 KB  |
  | + the `parley` measurer     | 982 KB | 416 KB  |

  `parley` alone would take most of the web's 350 KB budget. That confirms the browser measurer for the web, with `parley` as an opt-in feature for desktop and SSR.
- **Typing stays local.** A keystroke keeps the block's old layout. Relayout then re-itemizes only the words around the edit, and re-breaks lines from the line before it. It stops as soon as a line starts where an old one did over unchanged text, because greedy line breaking depends only on where a line starts, so every later line is reused. A keystroke in the middle of a 100 KB paragraph costs about 25 µs. Bidi paragraphs, and edits that insert right-to-left text, lay out from scratch, because their levels depend on the whole paragraph.
- **Structural edits stay local too.** A structural commit re-flattens only the root-level subtrees that its steps name, and splices them into place.
- **Server rendering.** [The SSR CSS contract](./ssr-css-contract.md) lists what the HTML pins, and what the web host must do, so the HTML wraps exactly where the canvas does.

## Decision 5: a display list as the rendering contract

Layout produces a `DisplayList` of positioned text runs, rectangles (selection, highlights, carets), decorations (underline, strike, squiggle), and placeholder slots for host-rendered node views. The display list is the only thing painters see:

- **Canvas2D** (web): `fillText` for each run, with dirty-rectangle repaint.
- **SVG string** (SSR, export, thumbnails): the same runs as `<text>` elements.
- **Semantic HTML** (SSR, no-JavaScript readers, SEO): flowing HTML whose CSS matches the editor's theme, so the canvas hydrates in place.
- **vello** (desktop), **CoreGraphics** (iOS), **Android Canvas**, and **Flutter Canvas**.

Custom node views, such as a React component inside the document, get a reserved rectangle in the display list. The host positions a real native or DOM view over it. That lets embeds stay ordinary components while text stays on the canvas.

Items carry the block they belong to. `DisplayList::damage` compares two lists block by block and returns the rectangles to repaint. Painting those rectangles from the new list reproduces it exactly.

## Decision 6: one input state machine, native input adapters

Rust holds the input state: IME composition range, pending marks, the keymap, and input rules. Each host adapts its native text-input API to that one machine:

- **Web:** `EditContext` where it exists, which today is Chromium only. Elsewhere, a hidden `<textarea>` that diffs its content after every input event. A kill switch forces the fallback.
- **iOS:** a `UIView` implementing `UITextInput` and `UITextInteraction`, so autocorrect, dictation, and the selection loupe all work.
- **Android:** a `View` with a custom `InputConnection` (`BaseInputConnection`), tested against Gboard and Samsung Keyboard.
- **Flutter:** `DeltaTextInputClient`.
- **React Native and Lynx:** wrap the iOS and Android native views. Avoid a WebView.
- **Desktop:** IME events from `winit` (preedit and commit).

## Decision 7: extensions, so you pay only for what you use

An extension contributes any of:

- schema items (blocks, marks, embeds);
- commands, keymaps, and input rules, such as `##` → heading;
- paste rules;
- a suggestion trigger, such as `@` or `/`;
- decorations;
- node-view slots.

In Rust, each extension sits behind a Cargo feature in a "kit" crate, so code you don't register isn't compiled in. JavaScript and Dart can add commands, keymaps, and node views at runtime. Schema items stay compiled in, which keeps documents verifiable.

## Decision 8: headless, with typed plugin APIs

canvist ships no toolbar, menus, or other UI. Every capability an extension adds is reachable through explicit calls:

- commands;
- `can` checks (would this command run right now?);
- active-state queries (is bold active? what level is this heading?);
- attribute getters.

Apps build their own buttons, bubble menus, and slash menus on top of those calls. Nothing happens by magic.

These calls are typed, so using an extension you didn't register fails to compile:

- **Rust:** registering an extension returns a typed handle (`let bold = builder.add(Bold::default())`), and its commands can only be called through that handle.
- **TypeScript:** the editor's type is inferred from the extensions array. `editor.commands.toggleBold()`, `editor.isActive("bold")`, and `editor.commands.setBlock("heading", { level: 2 })` type-check only when the matching extension is registered, and attribute values are checked too. An agent or a developer gets a compile error rather than a silent no-op.
- **Swift, Kotlin, and Dart:** the same handle-per-extension pattern, generated as far as each language allows.

To make this automatic, every extension declares machine-readable metadata. It lists the extension's commands with their argument and return types, its queries, its block, mark, and embed names, and its attribute schemas. Each binding generates its types from that metadata, and tests check the metadata against the real commands.

## Performance budgets

These budgets are enforced by benchmarks and checked in CI once each host exists.

| Metric                                            | Budget                                      | Today                |
| ------------------------------------------------- | ------------------------------------------- | -------------------- |
| Apply one keystroke (model), 100k blocks          | < 2 µs                                      | 0.29 µs              |
| Enter / Backspace across blocks, 100k blocks      | < 100 µs                                    | 45 µs                |
| Keystroke in a 100 KB paragraph                   | < 5 µs                                      | 0.94 µs              |
| Relayout one edited paragraph, 10k or 100k blocks | < 100 µs                                    | 2.5 µs               |
| Relayout after a keystroke in a 100 KB paragraph  | < 100 µs                                    | 25 µs                |
| Relayout after Enter, 10k / 100k blocks ¹         | < 200 µs / < 2 ms                           | 83 µs / 1.4 ms       |
| Hit test / caret rectangle                        | < 5 µs / < 2 µs                             | 0.8 µs / 0.01 µs     |
| Display list for one screen                       | < 100 µs                                    | 5 µs                 |
| Scroll one screen into unmeasured blocks          | < 1 ms                                      | 53 µs                |
| Keystroke → paint on the web (Event Timing), p95  | < 8 ms                                      | (M3)                 |
| Typing burst: 1,000 key events in one task        | no crash, no dropped input, ≤ 2 frames late | model passes 20k ops |
| Open a 10k-paragraph document → first paint       | < 50 ms                                     | layout 0.6 ms (M2)   |
| Web bundle (core + StarterKit), gzipped           | < 350 KB                                    | (M3)                 |

For reference, the best public number the research found is Lexical issue #7422: about 1.8 ms per keystroke at 5,000 paragraphs, spent copying state.

¹ **Recorded exception: Enter grows with the document.** Layout re-flattens only the root-level subtrees a structural commit changed and reuses every other block's row and layout. But rows are stored in one array, so splicing a row in shifts the index of every row after it. That work is plain memory moves and integer updates, with no hashing, styling, or measuring, and costs about 10 ns per row. Chunked row storage would make it `O(log n)`, and is the follow-up if documents grow past 100k blocks.

Layout's numbers use the deterministic `FixedMeasurer`, so they measure layout itself. Measuring with the browser or `parley` adds the host's own cost, but only for words the cache hasn't seen.

## Performance is a first-class primitive

Every change is judged on speed as well as correctness. The rules for every pull request:

1. **Budgets are tests.** Each hot path has a budget in the table above and a release-mode test in `tests/perf_budgets.rs` in its crate. `test:perf` runs these tests and the `perf` CI job fails when one is over budget. New hot paths get a budget test in the same PR that introduces them.
2. **Benchmarks accompany code.** A PR that adds or changes a hot path adds or extends `divan` benchmarks at realistic sizes: 100, 10k, and 100k blocks, long paragraphs, and heavy formatting.
3. **Numbers in the description.** The PR description reports before and after benchmark numbers for every hot path it touches. A regression of more than 5% needs a written justification, or it doesn't merge.
4. **Complexity is part of the design.** Anything that runs per keystroke, per frame, or per visible line must be independent of document size, or bounded by the viewport. Structural edits may cost `O(siblings)`, never `O(document)`, without a recorded reason.
5. **Measure the real thing.** Each host adds end-to-end latency measurements (keystroke to paint, scroll, open) and a typing-burst stress test before it ships.

## Quality gates

- `cargo nextest`, doc tests, and property tests for every crate. Every step kind is checked for invariants, for undo restoring the original exactly, and for failed steps changing nothing.
- `test:perf`: release-mode budget tests, which fail CI when a hot path regresses past its budget.
- `monostyle check --fail-under 90` for both readability and complexity on every v2 crate (`lint:style`).
- `clippy` (pedantic) and `dprint`.
- Platform end-to-end suites: Playwright for the web, including the SSR → hydrate pixel diff; XCTest on the iOS Simulator; androidx.test on an emulator; Flutter `integration_test`.
