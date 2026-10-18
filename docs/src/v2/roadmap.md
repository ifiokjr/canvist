# canvist v2 roadmap

The work is split into milestones that can each ship as their own stack of pull requests. The milestones on one row of the dependency graph can run in parallel. [The architecture](./architecture.md) explains the decisions behind them.

```text
M0 model ─┬─ M1 editor engine ─┬─ M3 web host ─┬─ M7 Flutter / RN / Lynx
          │                    │               ├─ M9 docs site, AI skill, release
          ├─ M2 layout ────────┤               │
          │                    ├─ M5 desktop   │
          └─ M4 collaboration ─┴─ M6 iOS + Android ─┘
M8 TipTap-parity extensions: ongoing from M1 onwards
toolchain spike (UniFFI, simulators) and logo: in parallel, now
```

## M0: foundations (this PR)

- [x] `canvist_model`: schema, blocks, inline mark runs, embeds, invertible steps, transactions, point mapping, ProseMirror/TipTap JSON.
- [x] Property tests covering random edit sequences, rollback, and undo restoring the original exactly. A 20,000-keystroke typing burst.
- [x] `divan` benchmarks for every hot path, at 100, 10,000, and 100,000 blocks.
- [x] A `lint:style` gate: monostyle readability and complexity ≥ 90.

## M1: editor engine (`canvist_editor`)

- [x] **Commands:** insert text, delete backward and forward (by grapheme, word, line, or document), split and join that know about lists and quotes, wrap and lift, toggle mark, set block type, select all, and cut, copy, and paste from HTML, Markdown, and plain text.
- [x] **Selection motion:** grapheme, word, line, and document, plus block selection. Atomic blocks are selected as a whole when the caret reaches them.
- [ ] **Gap cursor** around atoms, for positions where text cannot go.
- [x] **Keymaps:** per-platform, so `Mod` maps to Cmd on Apple and Ctrl elsewhere. Input rules and paste rules.
- [x] **History:** grouping by time and by kind of edit, plus selection restore, behind a `History` trait the collaboration crate replaces.
- [x] **Typed extension API** (architecture Decision 8): typed handles in Rust, machine-readable command, query, and attribute metadata for codegen, chaining, and `can()` dry runs on exact model savepoints. The core stays headless. TypeScript generation from the metadata is M3.
- [x] **Extension trait and the StarterKit** (`canvist_extensions`): paragraph, heading 1–6, blockquote, code block, hard break, horizontal rule, bullet, ordered, and task lists, bold, italic, underline, strike, code, link, undo/redo, and trailing node, each behind a Cargo feature. The drop cursor moves to M3, because drag and drop belongs to the host.
- [x] **Tests:** headless, scripted keystrokes with JSON snapshots, for example `type("## Hi⏎- one⏎two⇥")`, property tests that random commands and keystroke scripts keep every invariant and undo and redo exactly, `divan` benchmarks, and `perf_budgets` tests: a keystroke through the whole StarterKit costs about 0.65 µs at 100,000 blocks.

## M2: layout (`canvist_layout`)

- [x] The batched `TextMeasurer` trait, a `parley` measurer (behind a feature), and a deterministic test measurer.
- [x] Line breaking (UAX #14), bidi (UAX #9), tabs, embeds, and grapheme-aware hit testing.
- [x] Caret and selection rectangles, including in RTL and mixed-direction text, and a `LayoutNavigator` that adapts to the editor's navigation trait.
- [x] Block layout cached by block, revision, style, and width, plus viewport virtualization with estimated heights and scroll anchoring.
- [x] The display list with dirty rectangles, and SVG and semantic HTML renderers for SSR, with [a CSS contract](./ssr-css-contract.md).
- [x] **Benchmarks and budget tests** at 100, 10k, and 100k blocks. A keystroke relayout takes 2.5 µs at any document size, and 25 µs in a 100 KB paragraph. A 10k-paragraph document reaches its first screen in 0.6 ms. Enter takes 83 µs at 10k blocks, and grows with the document by about 10 ns per row; chunked row storage is the follow-up.

## M3: web host (`canvist_web` + `@canvist/*` packages)

- wasm + Canvas2D painter with dirty rectangles and HiDPI support.
- Input through `EditContext` with a hidden-textarea fallback, IME on every major input method, clipboard, and drag and drop.
- An accessibility mirror DOM.
- `@canvist/core` (vanilla DOM), `@canvist/react`, `@canvist/vue`, and `@canvist/ssr`.
- TypeScript types generated from extension metadata. The editor's type is inferred from its extensions, so calling an unregistered extension's command is a compile error. A type-level test suite (`tsc --noEmit` with expected errors) enforces this.
- **Playwright:** functional suites, a keystroke-to-paint benchmark (Event Timing API), a typing-burst stress test, the SSR → hydrate no-jank pixel diff, and mobile web emulation.
- Examples: vanilla, React (Vite), Vue, Next.js SSR, and Nuxt SSR.

## M4: collaboration (`canvist_collab`)

- `yrs` 0.28 (bumped from 0.25). Steps map onto Y.Map / Y.Text / Y.Array, and remote events map back to steps.
- Awareness, with remote carets and selections, plus a collaborative undo manager.
- Compatibility tests against JavaScript Yjs peers, y-websocket, and Hocuspocus.
- Fuzzing: N peers make random edits with random delivery order and must converge. Concurrent-split data loss is measured.

## M5: native desktop (`canvist_desktop`)

- A `winit` window, a `vello` painter, the `parley` measurer, AccessKit, and IME.
- Example apps for macOS, Windows, and Linux, with CI builds for all three.

## M6: native iOS and Android

- UniFFI bindings (grown from the toolchain spike).
- iOS: `CanvistView` (`UITextInput`, CoreText measurer, CoreGraphics painter). A SwiftUI example and XCTest on the Simulator.
- Android: `CanvistView` (`InputConnection`, `Paint` measurer). A Compose example and instrumented tests on the emulator. Optionally run on the Seeker phone.

## M7: Flutter, React Native, and Lynx

- Flutter plugin. The rendering approach depends on the glyph-drawing prototype. Example app and `integration_test` on both simulators.
- React Native native component through uniffi-bindgen-react-native, with an example app.
- Lynx custom native element wrapping the iOS and Android views, with an example app.

## M8: TipTap-parity extensions

This list covers everything in TipTap v3's MIT set. See [the research brief](../research/editor-landscape.md#1-tiptap-v3-the-parity-checklist) for the full checklist.

- **Tables:** colspan and rowspan, column resizing, cell selection, merge and split.
- **Images** (resizable), YouTube, audio, and file handler.
- **Code blocks** with syntax highlighting. Details/summary. Math (KaTeX-equivalent layout, or a host-rendered node view).
- **Mention, emoji, and slash commands** on a shared `suggestion` primitive.
- **Text styling:** highlight, subscript, superscript, color, background color, font family, font size, line height, and text align.
- **Editor UI:** placeholder, character count, focus, invisible characters, typography, unique ID, find and replace, table of contents, drag handle, and bubble and floating menus.
- **Import/export:** Markdown and HTML, plus static rendering to HTML and SVG.
- **Stretch goals,** beyond TipTap's free tier: comments as anchored ranges, version snapshots, tracked changes, and paged layout.

## M9: documentation, AI readiness, and release

- **Documentation website:** interactive examples on every page, with a live editor running in the docs themselves.
- **AI skill:** a `canvist` agent skill that teaches coding agents to add canvist to any app on any platform, with an eval suite that scores agents on real integration tasks.
- **Final logo** (choose from the alternatives) and brand.
- **Release:** crates.io, npm/JSR, pub.dev, Swift Package, and Maven.
- **Cleanup:** delete the v1 crates (`canvist_core`, `canvist_render`, `canvist_wasm`) once M3 reaches parity with the v1 demo.

## Platform example matrix

| Platform            | Host crate / package       | Example                 | Automated tests                        |
| ------------------- | -------------------------- | ----------------------- | -------------------------------------- |
| Web (vanilla)       | `@canvist/core`            | `examples/web-vanilla`  | Playwright (Chromium, WebKit, Firefox) |
| React               | `@canvist/react`           | `examples/web-react`    | Playwright                             |
| Vue                 | `@canvist/vue`             | `examples/web-vue`      | Playwright                             |
| SSR                 | `@canvist/ssr`             | `examples/web-next-ssr` | Playwright pixel diff for hydration    |
| macOS/Windows/Linux | `canvist_desktop`          | `examples/desktop`      | CI builds, headless snapshot tests     |
| iOS (native)        | `CanvistKit` Swift package | `examples/ios`          | XCTest on the iOS Simulator            |
| Android (native)    | `canvist-android` AAR      | `examples/android`      | androidx.test on the emulator          |
| Flutter             | `canvist` (pub.dev)        | `examples/flutter`      | `integration_test` on both simulators  |
| React Native        | `@canvist/react-native`    | `examples/react-native` | Detox or Maestro on the simulators     |
| Lynx                | `@canvist/lynx`            | `examples/lynx`         | Lynx test runner on the simulators     |
