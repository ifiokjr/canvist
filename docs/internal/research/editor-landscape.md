# Editor landscape research brief

Research date: 2026-10-18. Version numbers were checked against crates.io, npm, and GitHub release APIs on that date. Items marked **(unverified)** come from memory or secondary sources and were not confirmed against a primary source in this pass.

This brief covers the architectural choices for canvist: a Rust-core rich-text editor rendered to a canvas, collaborative through `yrs`, shipping to the web, React, Vue, React Native, Flutter, iOS, and Android. It sets the TipTap feature-parity checklist and records what other editors teach about performance, input, and collaboration.

---

## 1. TipTap v3: the parity checklist

The current release is TipTap `v3.32.0`, from 2026-10-13 ([releases](https://github.com/ueberdosis/tiptap/releases)). TipTap v3 merged several small packages into kits: `@tiptap/extensions`, `@tiptap/extension-list`, `@tiptap/extension-table`, and `@tiptap/extension-text-style`. In 2025 it also open-sourced several former Pro extensions, including Details, Emoji, Mathematics, File Handler, Invisible Characters, Table of Contents, Unique ID, and Drag Handle. The source of truth for this list is the monorepo [`packages/`](https://github.com/ueberdosis/tiptap/tree/main/packages) folder and the docs repo [`src/content/editor/extensions`](https://github.com/ueberdosis/tiptap-docs/tree/main/src/content/editor/extensions).

### StarterKit (`@tiptap/starter-kit`)

Verified from [`starter-kit.ts`](https://github.com/ueberdosis/tiptap/blob/main/packages/starter-kit/src/starter-kit.ts):

- **Nodes:** Document, Paragraph, Text, Heading (levels 1 to 6), Blockquote, CodeBlock, HardBreak, HorizontalRule, BulletList, OrderedList, ListItem.
- **Marks:** Bold, Italic, Strike, Underline, Code, Link. Underline and Link are new in v3.
- **Functionality:** ListKeymap, Dropcursor, Gapcursor, UndoRedo (formerly History), TrailingNode.

### Nodes (MIT)

| Extension                                             | What it does                                                                                            |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Document / Paragraph / Text                           | Root node, the default block, and the inline text leaf                                                  |
| Heading                                               | Headings h1 to h6, configurable levels                                                                  |
| Blockquote                                            | Block quotation wrapper                                                                                 |
| BulletList / OrderedList / ListItem                   | List kit in `@tiptap/extension-list`. Ordered lists support a `start` attribute                         |
| TaskList / TaskItem                                   | Checkbox lists, which can be nested. Part of ListKit                                                    |
| CodeBlock                                             | Preformatted code block with a `language` attribute                                                     |
| CodeBlockLowlight                                     | CodeBlock with lowlight/highlight.js syntax decorations                                                 |
| HardBreak                                             | Inline line break (`<br>`)                                                                              |
| HorizontalRule                                        | `<hr>` divider                                                                                          |
| Image                                                 | Inline or block image with `src`, `alt`, `title`, and optional resizing                                 |
| Table / TableRow / TableHeader / TableCell (TableKit) | Tables with colspan/rowspan, column resizing, cell selection, and merge/split (from prosemirror-tables) |
| Details / DetailsSummary / DetailsContent             | Collapsible `<details>` blocks                                                                          |
| Mention                                               | Inline atom node triggered by `@`, built on `@tiptap/suggestion`                                        |
| Emoji                                                 | Inline emoji nodes with shortcodes, `:` suggestion, and fallback images                                 |
| Mathematics                                           | Inline and block LaTeX rendered with KaTeX                                                              |
| YouTube / Twitch                                      | Video embed nodes                                                                                       |
| Audio                                                 | Audio embed node. New in 3.x                                                                            |

### Marks (MIT)

Bold, Italic, Underline, Strike, Code, Link (autolink, paste-to-link, protocol validation), Highlight (multicolor), Subscript, Superscript, TextStyle (a carrier `<span style>` mark), and RubyText (HTML `<ruby>` annotations, new in 3.x).

### Functionality (MIT)

| Extension                                                              | What it does                                                                                 |
| ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| TextStyleKit: Color, BackgroundColor, FontFamily, FontSize, LineHeight | Style attributes stored on the TextStyle mark                                                |
| TextAlign                                                              | `textAlign` attribute on block nodes                                                         |
| Typography                                                             | Smart-quote and ellipsis input rules, plus `(c)` to ©, `->` to →, and similar                |
| Placeholder                                                            | Placeholder text for an empty document or empty node, implemented as decorations             |
| CharacterCount                                                         | Character and word counts with an optional limit                                             |
| Focus                                                                  | Adds a `has-focus` class to the focused node or nodes                                        |
| Selection                                                              | Keeps a visible selection decoration when the editor loses focus                             |
| Dropcursor / Gapcursor                                                 | Drop indicator, and a caret for positions where text cannot go (around atoms and tables)     |
| UndoRedo                                                               | Local undo/redo through prosemirror-history. Collaboration uses the Yjs undo manager instead |
| TrailingNode                                                           | Keeps an empty paragraph at the end of the document                                          |
| UniqueID                                                               | Stable ID attribute on configured node types                                                 |
| InvisibleCharacters                                                    | Shows spaces, hard breaks, and paragraph marks                                               |
| FileHandler                                                            | Drop and paste hooks for files                                                               |
| TableOfContents                                                        | Heading index with anchors and scroll tracking                                               |
| FindAndReplace                                                         | Search highlighting and replace. New in 3.x                                                  |
| NodeRange                                                              | Selection model that spans whole nodes                                                       |
| BubbleMenu / FloatingMenu                                              | Menus positioned at the selection or on empty lines (Floating UI)                            |
| DragHandle (+ React/Vue)                                               | Block drag handle                                                                            |
| Collaboration                                                          | Yjs binding through `@tiptap/y-tiptap`, a fork of y-prosemirror                              |
| CollaborationCaret                                                     | Remote carets and selections from Yjs awareness. In v2 this was CollaborationCursor          |
| Suggestion utility                                                     | Trigger-character popup engine used by Mention, Emoji, and slash commands                    |
| Markdown (`@tiptap/markdown`)                                          | Markdown parse and serialize                                                                 |
| HTML / static-renderer                                                 | Render JSON to HTML, React, or a string without an editor instance                           |

### Paid (Start, Team, and add-on plans)

From the frontmatter `tags` in the docs repo:

- **Comments (Start).** Threaded inline comments tied to TipTap Cloud.
- **Snapshot (Start).** Version history.
- **Snapshot Compare (Team).** Diff between versions.
- **Import (Start, beta).** Converts docx, odt, and markdown into TipTap.
- **Export (Start, beta).** Converts TipTap into docx, odt, and markdown.
- **Pages (Team).** Page-based layout with margins, headers, and page breaks.
- **Paste Handler (Team).** Cleans paste from Word, Excel, and Google Docs.
- **Tracked Changes (add-on).** Suggestion mode.
- **AI Generation (Start).** Autocomplete and commands.
- **AI Toolkit (add-on).** Agent read and edit primitives; `@tiptap/ai-toolkit` 0.4.0 is a client SDK for a server service.
- **Collaboration Cloud / Hocuspocus.** Hocuspocus is the MIT Yjs server. The cloud service is paid.

Earlier paid items such as AI Assistant, AI Changes, AI Suggestion, and Content AI now appear to be folded into AI Generation and AI Toolkit **(unverified)**.

**Parity note:** canvist needs a `suggestion` primitive (trigger characters with an anchored popup), an `input rule` primitive (regex on typed text), and a `keymap` primitive. Mention, Emoji, Typography, and slash menus all compose from these three.

---

## 2. The ProseMirror model and its limits

ProseMirror is now hosted at `code.haverbeke.berlin`; the GitHub mirrors are archived ([prosemirror-view](https://github.com/ProseMirror/prosemirror-view)). The latest versions are prosemirror-view 1.42.6 and prosemirror-model 1.25.12.

### Document model

- **Schema.** Node and mark specs. Each node has a content expression, which is a regular language over node names or groups such as `paragraph+`, `(paragraph | heading)*`, `inline*`, or `image{2,4}`. Each node also has `marks`, `group`, `inline`, `atom`, `defining`, and `isolating` flags. Content matching runs on a compiled finite automaton, `ContentMatch` ([guide](https://prosemirror.net/docs/guide/#schema)).
- **Nodes are immutable and persistent.** A `Fragment` is an array of children, and `Node.text` holds flat strings. Marks are a sorted set on each inline node, so the document is a tree of blocks over a flat inline sequence.
- **Positions are integer token offsets.** Each text character (UTF-16 code unit) counts as 1, each leaf node counts as 1, and each non-leaf node adds 1 for its opening token and 1 for its closing token. `doc.resolve(pos)` returns a `ResolvedPos` with depth, parent, index, and offset. Resolution costs O(depth × log children), and ProseMirror caches the result ([guide: indexing](https://prosemirror.net/docs/guide/#doc.indexing)).

### Transform layer

- **Steps.** `ReplaceStep(from, to, slice, structure)`, `ReplaceAroundStep(from, to, gapFrom, gapTo, slice, insert, structure)` (used for wrap and lift), `AddMarkStep`, `RemoveMarkStep`, `AddNodeMarkStep`, `RemoveNodeMarkStep`, `AttrStep`, and `DocAttrStep`. Every step serializes to JSON and inverts itself ([transform ref](https://prosemirror.net/docs/ref/#transform)).
- **Mapping.** Each step produces a `StepMap` of `[start, oldSize, newSize]` ranges. A `Mapping` composes StepMaps with mirror pairs, so that rebasing works. `map(pos, assoc)` and `mapResult` report whether the position was deleted.
- **Transactions.** A `Transaction` is a `Transform` plus a selection, stored marks, metadata, and a time. `EditorState.apply` runs each plugin's `state.apply`. Plugin `filterTransaction` and `appendTransaction` hooks allow veto and follow-up edits.
- **Collaboration.** `prosemirror-collab` uses central-authority step rebasing, which is OT-like rather than a CRDT. When the server rejects a client's steps, the client rebases them by mapping them through the confirmed steps.

### View

- **Rendering.** The view uses `contenteditable` with a `ViewDesc` tree that mirrors the document. On update it diffs the old and new doc and redraws only the changed nodes. A `MutationObserver` together with `DOMObserver` reads browser-made changes (IME, spellcheck, autocorrect) and parses them back into steps using `parseBetween` and a doc diff. Most of ProseMirror's complexity lives in this round trip.
- **Decorations.** Inline, node, and widget decorations are held in a `DecorationSet`, a persistent tree of decorations grouped by node. `DecorationSet.map` updates them through a transaction.

### Known performance limits (from the forum and issue history; magnitudes vary)

- There is **no virtualization**. Every node has DOM, and very long documents (hundreds of thousands of nodes) slow layout, `posAtCoords`, and `coordsAtPos`. Even so, ProseMirror handles roughly 1 million characters of plain paragraphs well. In the [Lexical #7422 thread](https://github.com/facebook/lexical/issues/7422), the reporter called a react-prosemirror demo with _Moby Dick_ (about 1.1M characters) "smooth".
- **Decorations are the most common hot spot.** Plugins that rebuild a `DecorationSet` from scratch on every transaction (syntax highlighting, spellcheck, search, comments, collaboration carets) cost O(document) per keystroke. The fix is incremental `map` with local recompute. Thousands of inline decorations also fragment text nodes in the DOM.
- **Many plugins each scanning the document** in `appendTransaction` or `state.apply` (for example UniqueID or TableOfContents scanning `doc.descendants` on every change) add up to O(plugins × document) work per keystroke.
- **Node views written in React or Vue** (TipTap's `ReactNodeViewRenderer`) add reconciliation cost per node, and TipTap documents this. TipTap v3 added `shouldRerenderOnTransaction: false` and selectors to reduce React re-renders.
- **Position fragility.** Integer positions shift on every edit, so anything stored outside the editor needs a mapping or a CRDT anchor.

### y-prosemirror

**v1** (`y-prosemirror@1.3.7`, Yjs 13) maps each PM node to a `Y.XmlElement`, inline content to `Y.XmlText` (`Y.Text`) with marks as formatting attributes, and the document to a `Y.XmlFragment`. Known problems:

1. **Remote updates replace the whole document.** `_typeChanged` and `_forceRerender` call `tr.replace(0, doc.content.size, …)` ([sync-plugin.js, v1](https://github.com/yjs/y-prosemirror/blob/v1/src/plugins/sync-plugin.js)). Plugins therefore see a single full-document step, `Mapping` cannot map decorations or positions through a remote change, and both selection and decorations have to be recomputed through relative positions. Each remote keystroke costs O(document) in PM diffing.
2. **Undo must use `Y.UndoManager`.** prosemirror-history is incompatible.
3. **Schema-invalid merged states are fixed by deleting nodes.** For example, two clients that concurrently empty a `paragraph+` container cause the whole parent to be dropped ([CAVEATS.md](https://github.com/yjs/y-prosemirror/blob/master/CAVEATS.md)).
4. **Overlapping marks of the same type** (comments) need hashed keys (`comment--<hash>`).
5. **Initial-content duplication.** When two clients seed the same default document, both copies get inserted.

**v2** (`@y/prosemirror@2.0.0-15`, still prerelease, on Yjs v14 `@y/y`, which is at RC 29 as of 2026-10-15 per [yjs releases](https://github.com/yjs/yjs/releases)) is a rewrite. It is delta-based, has an attribution manager for suggestion mode and version diffs, flattens inline text into the parent type, and uses a fix loop for schema convergence. Its README still tells production users to stay on v1.

**Lesson for canvist:** keep a single source of truth. The ProseMirror plus Yjs pairing keeps two document models and an O(n) reconciliation between them. canvist should make the CRDT the model, or derive the model from the CRDT incrementally, rather than syncing two trees.

---

## 3. Other editors: what each does well and badly

### Quill 2 (Parchment and Delta)

Quill 2.0.3 shipped in November 2024, and the repository has seen little activity since mid-2025 ([repo](https://github.com/slab/quill)). The document is a **Delta**: a flat list of `insert`, `retain`, and `delete` ops with attribute maps, closed under `compose`, `transform`, and `invert` (OT). An embed is an `insert` of an object. Block formats live on the `\n` character that ends each line. **Parchment** is the DOM model, built from blots (Scroll, Block, Inline, Embed, Text).

- **Good:** the model is tiny and linear, and it maps one to one onto `Y.Text` (y-quill is trivial). Positions are plain character indexes. OT is cheap.
- **Bad:** there is no real tree structure. Tables and nested lists are awkward; they are flattened, with attributes for depth. DOM blots also mean no virtualization.
- **For canvist:** the "block attributes live on the newline" trick is exactly what `Y.Text` formatting does well.

### Lexical (Meta)

Lexical is at v0.52.0 ([releases](https://github.com/facebook/lexical/releases)). It keeps a `NodeMap` (key to node, mutable within an update), a **double-buffered EditorState** (current and pending), `$`-prefixed functions that are only valid inside `editor.update()`, dirty-node tracking, and a **reconciler** that diffs dirty nodes into DOM. TextNodes are split per format run.

- **Good:** fast for small and medium documents, a clean plugin API, and small core (around 22 KB gzip, **unverified**).
- **Bad:** the maintainer acknowledges degradation at hundreds of thousands of nodes. In a 2026 profile of [#7422](https://github.com/facebook/lexical/issues/7422) (5k paragraphs), `cloneEditorState` took about 23% of a 1.8 ms keystroke and `reconcileRoot` about 74%, because both are O(N) map copies per update. A copy-on-write `GenMap` was proposed as the fix. History stores whole EditorState snapshots. There is no virtualization. `@lexical/yjs` maps elements to `Y.XmlText` with embedded child types, which has historically been fragile.

### Slate

Slate is at 0.146 on npm. It uses a plain JSON tree, **path-based positions** (`[0, 2, 1]` plus an offset), and nine primitive ops (`insert_text`, `remove_text`, `insert_node`, `remove_node`, `split_node`, `merge_node`, `move_node`, `set_node`, `set_selection`). Rendering goes through React.

- **Good:** flexible, with an easy mental model.
- **Bad:** React rendering costs dominate on large documents, normalization is unbounded, and IME and Android handling have been a long-running source of bugs. slate-yjs exists as a third-party binding. Recent slate-react releases added chunked rendering for large documents **(unverified)**.

### BlockSuite / AFFiNE

BlockSuite is **Yjs-native** and block-based. A document is a `Y.Doc` holding a flat `blocks` `Y.Map` keyed by block ID. Each block is a `Y.Map` with `sys:flavour`, `sys:children` (a `Y.Array` of IDs), and `prop:*` fields, and rich text is one `Y.Text` per block. The inline editor renders each block's `Y.Text` delta into its own small contenteditable. AFFiNE adds an "edgeless" canvas (whiteboard) mode on the same model.

- **Good:** there is no second model to sync, so remote edits touch only the affected blocks. Block moves are ID moves, and per-block rendering keeps cross-block selection explicit.
- **Bad:** cross-block selection and IME across blocks are hard. Very large block maps make Yjs document load and garbage collection heavy. The repository says it "will undergo major changes" and has suspended PRs ([README](https://github.com/toeverything/blocksuite)); its last tagged release was v0.22.4 in July 2025.

### BlockNote

BlockNote is at v0.56.0 ([releases](https://github.com/TypeCellOS/BlockNote/releases)). It is a block-structured UX on TipTap and ProseMirror (a `blockContainer` / `blockGroup` schema) with collaboration through y-prosemirror. It inherits ProseMirror's limits and adds a JSON block API on top. The y-prosemirror v2 attribution and suggestion work is being developed together with BlockNote (see the `blocknote-demo` branch).

### Notion

Every block is a record with an `id`, `type`, `properties` (rich text stored as arrays of `[text, [[annotation, …]]]`), `content` (ordered child IDs), and a `parent` pointer, which permissions inherit from ([data model](https://www.notion.com/blog/data-model-behind-notion)). Writes historically went through server-side ops on records with last-writer-wins at the property level. Offline mode migrates pages marked for offline use to a **new CRDT data model** for rich-text conflict resolution ([offline post](https://www.notion.com/blog/how-we-made-notion-available-offline)). The CRDT implementation details are not public in the excerpt we read.

- **Lesson:** block identity and the tree structure live in the record store, while text-level merging needs a CRDT.

### Google Docs on canvas

Google moved Docs from HTML to canvas rendering in May 2021 to improve "performance and consistency across platforms" ([Workspace Updates](https://workspaceupdates.googleblog.com/2021/05/Google-Docs-Canvas-Based-Rendering-Update.html)). The move broke DOM-scraping Chrome extensions. Input goes to a hidden, offscreen contenteditable iframe or element that captures keystrokes and IME. Screen-reader support comes from an opt-in "screen reader support" mode that keeps an accessible DOM and live regions in sync, plus braille support **(implementation details unverified; based on public behavior)**. Docs uses its own text layout engine, while fonts are still loaded as web fonts.

- **Lesson:** a canvas editor is viable at the largest scale, but accessibility and input need a parallel invisible DOM.

### Xi editor, Zed, and GPUI

- **Xi** ([retrospective](https://raphlinus.github.io/xi/2020/06/27/xi-retrospective.html)) found that a rope (a B-tree of string chunks with summarized metrics) was the right core data structure. Its process-separated JSON-RPC architecture, and CRDT-for-everything (including plugins), were over-engineered: async plugin edits needed CRDT merging, which made simple features hard. The project is effectively dormant; its last push was in 2024.
- **Zed** ([SumTree and rope](https://zed.dev/blog/zed-decoded-rope-sumtree), [CRDTs](https://zed.dev/blog/crdts)) builds everything on a **SumTree**, a B+ tree whose nodes carry monoidal summaries (bytes, chars, UTF-16 code units, line and column, max line length). It seeks along any dimension in O(log n). The buffer is a CRDT: fragments carry insertion IDs and Lamport timestamps, and **Anchors** are `(insertion_id, offset, bias)`, which resolve to positions in O(log n) and stay stable across concurrent edits. GPUI shapes text through platform APIs (CoreText, DirectWrite, cosmic-text on Linux) and renders from a GPU glyph atlas.
- **Lesson:** use one tree with multiple metrics for offset conversion, anchors for anything persistent, and keep CRDT complexity out of the plugin and extension surface.

---

## 4. Text input

### EditContext (web)

The status per [MDN browser-compat-data](https://github.com/mdn/browser-compat-data/blob/main/api/EditContext.json) (fetched 2026-10-18):

- **Chromium:** shipped in Chrome, Edge, and Opera 121, including Android and WebView ([chromestatus](https://chromestatus.com/feature/5041440373604352)).
- **Firefox:** not shipped ([bug 1904161](https://bugzil.la/1904161)). Mozilla's position is _positive_, with compatibility and accessibility concerns ([mozilla/standards-positions#199](https://github.com/mozilla/standards-positions/issues/199)).
- **Safari:** not shipped ([webkit.org/b/269922](https://webkit.org/b/269922)). The WebKit position request is still open ([WebKit/standards-positions#243](https://github.com/WebKit/standards-positions/issues/243)).

The feature is marked experimental and is not Baseline. This means **iOS has no EditContext in any browser**, because every iOS browser uses WebKit.

**How it works.** You create `new EditContext({text, selectionStart, selectionEnd})` and assign it to `element.editContext`; the element can be a `<canvas>`. The browser routes IME and keyboard text into the context and fires `textupdate` (with `updateRangeStart`, `updateRangeEnd`, `text`, and selection), `textformatupdate` (composition underline styles), `characterboundsupdate` (the browser asks for glyph rectangles so it can position the IME window), and `compositionstart` and `compositionend`. The editor calls `updateText`, `updateSelection`, `updateControlBounds`, `updateSelectionBounds`, and `updateCharacterBounds`. The DOM is never mutated by the browser. ([Chrome blog](https://developer.chrome.com/blog/introducing-editcontext-api), [Edge blog](https://blogs.windows.com/msedgedev/2024/02/13/custom-web-editing-experiences-with-editcontext/))

**Maturity signal.** CodeMirror 6 uses EditContext on Chrome 126 and later. Its changelog records repeated workarounds: document corruption (6.28.4), a Chrome memory leak (6.28.5), wrong composition positions causing crashes, broken spellcheck corrections, read-only bypass, and Samsung-keyboard autocompletion issues ([codemirror/view CHANGELOG](https://github.com/codemirror/view/blob/main/CHANGELOG.md)).

**Conclusion:** EditContext is a good Chromium path but needs a full fallback and a kill switch.

### Hidden textarea fallback

Use this on Safari, Firefox, and as the fallback on Chromium. Keep one focused, transparent `<textarea>` (or a contenteditable `div`) positioned at the caret so the IME candidate window, the iOS magnifier, and the Android keyboard anchor correctly.

- Mirror a **window of surrounding text**, for example the current paragraph or about 1–2k characters around the caret, together with the selection. Without real context, autocorrect, predictive text, and the Android IME's re-composition break.
- Read changes from `beforeinput` (`inputType`, `getTargetRanges()`, `data`) plus `input` and composition events. Diff the textarea value against the last value you mirrored as a fallback, because Android composition often fires `beforeinput` events that cannot be cancelled. Apply the result as editor operations, then re-sync the textarea. Never re-sync while a composition is active.
- This is the CodeMirror 5, Monaco, Google Docs, and Figma-text approach.

### IME and composition pitfalls

- Never mutate the textarea value or move the selection during a composition; doing so cancels it on Safari and Chrome.
- `compositionend` ordering differs across browsers. Safari fires `keydown` 229 after `compositionend`. Enter that confirms a candidate must not insert a newline, so check `isComposing` or `keyCode === 229`.
- Korean (Hangul) IMEs commit per syllable and recompose on backspace. The Japanese IME reconverts a selected range. Render composition text inline with an underline in your own layout, using the formatting hints from `textformatupdate` where available.
- Dead keys and Option-key accents on macOS behave as single-character compositions.
- In collaboration, treat the composition range as provisional local state. Either suspend applying remote ops into the composing range or rebase the composition anchor through a sticky index.

### Mobile web

- **Android Chrome with Gboard.** Gboard composes the whole word under the caret (`setComposingRegion`), so tapping into an existing word starts a composition over it. Backspace often arrives as `deleteContentBackward` without a reliable `keydown` (`keyCode` 229), and many `beforeinput` events from composition cannot be cancelled. Autocorrect replaces earlier text with an `insertReplacementText` or composition update. The keyboard needs accurate surrounding text, or suggestions and swipe input misbehave. Samsung Keyboard and SwiftKey differ again. ProseMirror's and Slate's Android handling are notorious for these reasons.
- **iOS Safari.** `insertReplacementText` handles autocorrect. Predictive-bar insertions can add a trailing space, and smart punctuation replaces text. Dictation streams as composition, with tentative text replaced several times, and it can insert the 0xFFFC object replacement placeholder. The caret, loupe, and selection handles belong to the hidden element, so the textarea's selection must match the visual selection, or canvist must hide the native handles and draw its own. Focus generally needs a user gesture to raise the keyboard, and `visualViewport` resize events signal the keyboard height.
- **Voice dictation, on both platforms,** produces multi-word insertions and later replacements; it must not be treated as typing for input rules.

### Native input APIs

- **iOS: `UITextInput` and `UIKeyInput`.** Implement `insertText`, `deleteBackward`, `setMarkedText(_:selectedRange:)`, `unmarkText`, `markedTextRange`, `selectedTextRange`, `text(in:)`, `replace(_:withText:)`, and custom `UITextPosition` and `UITextRange` subclasses. Also implement the geometry methods (`firstRect(for:)`, `caretRect(for:)`, `selectionRects(for:)`, `closestPosition(to:)`), a `tokenizer` (`UITextInputStringTokenizer`), and the `inputDelegate` notifications `textWillChange`/`textDidChange` and `selectionWillChange`/`selectionDidChange`. Missing these notifications desynchronizes autocorrect.
- **iOS: `UITextInteraction`** (iOS 13) adds the system selection gestures, loupe, and handles to a custom `UITextInput` view. iOS 17 split these into `UITextSelectionDisplayInteraction` and `UITextLoupeSession`. Apple Intelligence Writing Tools (iOS 18) and Scribble (`UIIndirectScribbleInteraction`) work through these protocols **(Writing Tools specifics unverified)**.
- **Android:** override `View.onCreateInputConnection(EditorInfo)` and return a `BaseInputConnection` subclass.
  - `EditorInfo` carries `inputType`, `imeOptions`, `initialSelStart`/`initialSelEnd`, and `setInitialSurroundingText` (API 30 and later).
  - `InputConnection` covers `commitText`, `setComposingText`, `setComposingRegion`, `finishComposingText`, `deleteSurroundingText` and `deleteSurroundingTextInCodePoints`, `getTextBeforeCursor`, `getTextAfterCursor`, `getSelectedText`, `getExtractedText`, `beginBatchEdit`/`endBatchEdit`, `sendKeyEvent`, `performEditorAction`, `requestCursorUpdates`, and `performHandwritingGesture` (API 34 and later, stylus).
  - The editor **must** call `InputMethodManager.updateSelection(selStart, selEnd, candStart, candEnd)` after every change and `updateCursorAnchorInfo` for IME window placement, and call `restartInput` after external (remote) changes near the caret.
  - `BaseInputConnection` with a backing `Editable` is the easy path. A custom connection that serves a window of text and translates directly into editor operations is the robust path.
- **Flutter:** `TextInputClient` receives a full `TextEditingValue` (text, selection, composing range) on each change. **`DeltaTextInputClient`** (with `TextInputConfiguration(enableDeltaModel: true)`) receives `updateEditingValueWithDeltas(List<TextEditingDelta>)`: `TextEditingDeltaInsertion`, `Deletion`, `Replacement`, and `NonTextUpdate` ([API](https://api.flutter.dev/flutter/services/DeltaTextInputClient-class.html)). The editor sends `setEditingState` with a serialized window, using placeholder characters for non-text nodes. super_editor uses this pattern. `ScribbleClient` adds iPad Pencil support. The IME gets its position from `setEditableSizeAndTransform` and `setCaretRect` / `setComposingRect`.
- **React Native.** The options are: (a) a native Fabric component per platform that wraps the iOS `UITextInput` view and the Android `InputConnection` view, which is best; (b) a WebView running a web editor, which is what 10tap-editor does with TipTap and is simple but slow and non-native; (c) driving a hidden RN `<TextInput>`, which is not viable for composition fidelity. For rendering, react-native-skia exposes glyph drawing, but input still needs a native view.

---

## 5. Rust text shaping and layout (October 2026)

| Crate                                                           | Version (date)      | Role                                                                                                        | Notes                                                                                                                                                                                                                                                                                                                        |
| --------------------------------------------------------------- | ------------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [parley](https://crates.io/crates/parley)                       | 0.12.0 (2026-10-09) | Rich-text layout: bidi, line breaking, alignment, cursor/selection geometry, `PlainEditor`                  | Split into `parley_engine` + `parlance` (property types). The engine depends on **harfrust 0.12, skrifa 0.44, icu_segmenter, icu_properties, icu_normalizer 2.3**, `parley_data`, and `parley_emoji`. It has hit-testing, `Cursor`, and `Selection` APIs for editing. This is the Linebender stack used by Xilem and Masonry |
| [fontique](https://crates.io/crates/fontique)                   | 0.12.0              | Font enumeration and fallback                                                                               | Optional backends: CoreText, DirectWrite, fontconfig, Android XML. On wasm it uses registered blobs only                                                                                                                                                                                                                     |
| [harfrust](https://crates.io/crates/harfrust)                   | 0.14.0 (2026-10-04) | HarfBuzz shaping port, pure Rust                                                                            | Lives under `harfbuzz/harfrust` and replaces rustybuzz, whose last release was 0.20.1 in November 2024. Built on `read-fonts`                                                                                                                                                                                                |
| [skrifa](https://crates.io/crates/skrifa) / read-fonts          | 0.48.0 / 0.45.1     | Font metadata, outlines, hinting, and COLR from Google Fontations                                           | Font parsing for Chrome's Skia (Fontations); actively released                                                                                                                                                                                                                                                               |
| [swash](https://crates.io/crates/swash)                         | 0.2.10 (2026-07)    | Older all-in-one shaper and scaler/rasterizer                                                               | Still maintained, but Linebender's direction is harfrust + skrifa (+ vello/tiny-skia)                                                                                                                                                                                                                                        |
| [cosmic-text](https://crates.io/crates/cosmic-text)             | 0.19.0 (2026-04)    | Full text stack (fontdb, shaping, layout, editor, swash raster)                                             | Now depends on **harfrust 0.5** + skrifa 0.40. Uses `unicode-bidi` and `unicode-linebreak`. Simpler API, but less rich-text oriented than parley                                                                                                                                                                             |
| [icu_segmenter](https://crates.io/crates/icu_segmenter)         | 2.3.0               | UAX #14 lines and UAX #29 graphemes/words/sentences, with dictionary/LSTM for Thai, Lao, Khmer, and Burmese | Compiled data adds binary weight. The LSTM and dictionary models are large; leave them out unless SE-Asian word breaks are needed                                                                                                                                                                                            |
| [unicode-bidi](https://crates.io/crates/unicode-bidi)           | 0.3.18 (2024-12)    | UAX #9                                                                                                      | Stable, low churn                                                                                                                                                                                                                                                                                                            |
| [unicode-linebreak](https://crates.io/crates/unicode-linebreak) | 0.1.5 (2023)        | UAX #14 only                                                                                                | Small. No updates since 2023, so check the Unicode version                                                                                                                                                                                                                                                                   |
| unicode-segmentation                                            | 1.13.3              | UAX #29                                                                                                     | Small; fine for graphemes and words                                                                                                                                                                                                                                                                                          |

**Wasm viability.** All of the above are pure Rust and build for `wasm32-unknown-unknown`. On wasm, fontique has no system font access, so fonts must be supplied as bytes (fetched web fonts, or Local Font Access on Chromium only, behind a permission prompt). **We did not measure binary size for this brief.** Expect the shaping and layout core to land in the hundreds of KB after `wasm-opt -Oz` **(unverified)**; the main variables are ICU4X data, emoji data, and harfrust's tables. Track it in CI with `twiggy` and `cargo bloat`.

**Rendering is a separate problem from shaping.** Shaped output is glyph IDs and positions, and each platform's ability to draw glyph IDs differs:

- **Canvas2D:** there is no API that draws by glyph ID. The choices are to draw skrifa outlines as cached `Path2D` objects or rasterize into a glyph atlas (vello_cpu or tiny-skia, or a WebGL/WebGPU atlas), or to call `fillText` per cluster with the same web font and accept subtle mismatch.
- **iOS:** `CTFontDrawGlyphs`.
- **Android:** `Canvas.drawGlyphs`, API 31 and later **(verify the minimum API you support)**.
- **react-native-skia:** a glyphs primitive.
- **Flutter `dart:ui`:** no public glyph-run drawing API as far as we know **(unverified)**. Flutter has to either draw a Rust-rendered texture or bitmap, or let Flutter's own Paragraph lay out the text, which loses layout determinism.

**HTML-in-Canvas** (WICG, `drawElementImage`) is in a Chrome origin trial with API changes in Chrome 150 and 155 ([Chrome blog](https://developer.chrome.com/blog/html-in-canvas-ot-changes)). It could eventually let a canvas draw real DOM text, keeping accessibility and native text rendering. It is Chromium-only and experimental, so watch it and do not depend on it.

---

## 6. Bindings

- **UniFFI** ([mozilla/uniffi-rs](https://github.com/mozilla/uniffi-rs)) is at 0.32.2 (2026-09-23) and still pre-1.0, with breaking changes between minor versions. It has proc-macro definitions (`#[uniffi::export]`, `Record`, `Object`, `Enum`), async support, and callback interfaces/traits. It produces first-party Kotlin, Swift, Python, and Ruby bindings. It is mature enough for Firefox, Matrix SDK, and Bitwarden-scale use.
- **uniffi-bindgen-react-native** ([repo](https://github.com/jhugman/uniffi-bindgen-react-native)) is at `0.31.0-6` (2026-09-25). From the same UniFFI annotations it generates TypeScript plus **JSI C++ and a Turbo Module** for React Native, a **wasm-bindgen crate for the web**, and an N-API runtime (`@ubjs/node`) for Node. It plans to rename itself to `uniffi-bindgen-javascript`. Users include `react-native-matrix-sdk`. This means one annotation set can cover Swift, Kotlin, RN, and web, which suits canvist well. The version must track the UniFFI version it supports.
- **flutter_rust_bridge v2** ([repo](https://github.com/fzyzcjy/flutter_rust_bridge)) is at 2.13.0 stable on crates.io (2026-09), with 2.14 in beta. It uses codegen from plain Rust signatures, supports opaque types, streams, `StreamSink`, async, and callbacks into Dart, and works on the web through wasm. It is mature and widely used. Dart FFI is synchronous-capable, and the bridge can run Rust on a worker thread pool.
- **wasm-bindgen** is at 0.2.129. Size tips:
  - Use `opt-level = "z"` or `"s"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, and `strip = true`.
  - Run `wasm-opt -Oz` (binaryen) and avoid `format!` and `Debug` in release builds; `core::fmt` is costly.
  - Keep generic monomorphization out of FFI surfaces, gate ICU4X data and emoji tables behind features, and avoid `serde_json` on the hot path (pass typed arrays or a compact binary diff).
  - Consider `wasm-bindgen --split-linked-modules` and lazy-load optional features such as math and syntax highlighting as separate modules.
  - Measure with `twiggy top` and `twiggy dominators`. Default to the `wasm32-unknown-unknown` target with `web-sys` features enabled narrowly.

---

## 7. Accessibility for canvas editors

- **Web.** A canvas has no semantics, so keep an **accessibility mirror DOM**: an offscreen but not `display:none` region (`role="textbox"`, `aria-multiline="true"`) that holds the document as real elements (headings, lists, tables, links). Mirror only the viewport neighborhood plus structure for very large documents. Keep the DOM selection in sync with the editor selection so the screen reader announces caret moves. Use `aria-live` sparingly for remote edits.
  - Google Docs offers this behind an opt-in "screen reader support" mode, plus a braille mode. In 2021 it promised that screen readers, braille, and magnifiers would keep working after the canvas move ([Workspace Updates](https://workspaceupdates.googleblog.com/2021/05/Google-Docs-Canvas-Based-Rendering-Update.html)).
  - With EditContext, the element hosting the context gets focus and the accessibility tree comes from the mirror DOM; Mozilla's accessibility concern in [standards-positions#199](https://github.com/mozilla/standards-positions/issues/199) is exactly this gap.
  - Magnifiers and zoom follow the focused element's bounds, so keep the hidden input element positioned at the caret.
  - Respect `prefers-reduced-motion` and `forced-colors` (high-contrast mode) in the canvas palette.
- **iOS.** `UITextInput` views get VoiceOver text navigation if they also implement the `UIAccessibility` traits (`.allowsDirectInteraction` is not needed). Expose `accessibilityValue` and `accessibilityTextualContext`, plus `UIAccessibilityReadingContent` (`accessibilityLineNumber(for:)`, `accessibilityContent(forLineNumber:)`, `accessibilityFrame(forLineNumber:)`) for line reading. Represent links and embeds as `accessibilityElements` or custom rotors.
- **Android.** Provide an `AccessibilityNodeProvider` with virtual nodes. Set `AccessibilityNodeInfo` text, selection, and `isEditable`, use `setTextSelection` together with `ACTION_SET_SELECTION`, `ACTION_NEXT_AT_MOVEMENT_GRANULARITY`, and `ACTION_SET_TEXT`, and use `setTextSelectable`. TalkBack reads through these nodes. Emit `TYPE_VIEW_TEXT_CHANGED` and `TYPE_VIEW_TEXT_SELECTION_CHANGED` events.
- **Flutter.** `Semantics` with `textField: true`, `value`, `textSelection`, and `onSetSelection` / `onSetText` / `onMoveCursorForwardByCharacter`, and so on. Flutter generates the platform accessibility trees, including the web's DOM semantics tree.
- **Rust core.** The core should expose a platform-neutral **accessibility tree** (block roles, text runs, selection, bounds) that each adapter projects. **AccessKit** (Linebender) is the Rust standard for this on desktop and has a web adapter in development **(status unverified)**.

---

## 8. Measuring input latency

- **Field metrics.** The Event Timing API (`PerformanceEventTiming`) gives `processingStart`, `processingEnd`, and `duration`, which runs to the next paint and is rounded to 8 ms. INP (Interaction to Next Paint) became a Core Web Vital in March 2024, with "good" at 200 ms or less at p75. For an editor, the interactions that matter are `keydown`, `beforeinput`, and `input`. A long animation frame (LoAF) attributes slow frames to scripts.
- **Lab keypress-to-paint.** Synthesize real OS-level key events through CDP `Input.dispatchKeyEvent` or Playwright `keyboard.press`, mark `performance.now()` at the keydown handler, and record `requestAnimationFrame` plus a `MessageChannel` post or `requestPostAnimationFrame` to estimate presentation. Better still, use Chrome tracing (`devtools.timeline`, "EventDispatch" to "Commit" to "Present") or a camera or [Typometer](https://pavelfatin.com/typing-with-pleasure/) (Pavel Fatin's methodology), which captures true end-to-end screen latency. Report p50, p95, and p99 for document sizes of 1k, 10k, 100k, and 1M characters, separately for typing mid-paragraph, Enter, paste of 100 KB, select-all with a bold toggle, and scrolling.
- **Public numbers are scarce and informal:**
  - **Lexical:** about 1.8 ms CPU per keystroke at 5k paragraphs, dominated by O(N) state clones ([#7422](https://github.com/facebook/lexical/issues/7422), 2026). Pasting 500k characters took minutes, with about 1 s per keystroke afterwards, on 0.29 before fixes.
  - **Lexical vs ProseMirror stress test:** Emergence Engineering's hour-long test found Lexical faster initially but degrading over time, and ProseMirror slower but stable ([blog](https://emergence-engineering.com/blog/lexical-prosemirror-comparison)). It is one author's test from about 2024.
  - **Bundle-size comparisons (Aug 2026):** TipTap core about 34 KB gzip plus about 105 KB for StarterKit, Quill about 57 KB, Slate about 28 KB ([dev.to](https://dev.to/rudrabuilds/i-benchmarked-8-react-rich-text-editors-here-are-the-numbers-2lh5)). That post did not measure latency.
  - **CodeMirror 6** uses viewport-only rendering and handles multi-million-line documents. It is the reference for virtualization in the DOM world.
  - No standardized, reproducible editor latency benchmark exists. **canvist should publish one**, which is a credibility asset if the target is to "beat ProseMirror".

---

## Recommendations for canvist

These are opinionated. The repository currently pins `yrs ^0.25`; the latest release is **0.28.0** (2026-09-17), so upgrade before building the model.

1. **Data model: Yjs-native and block-structured, with no second tree to sync.**
   - Use a root `Y.Map` named `blocks` (`BlockId` to `Y.Map {type, attrs: Y.Map, children: Y.Array<BlockId>, text: Y.Text?}`) plus a root order array. This is the BlockSuite and Notion shape.
   - Rich text inside a block is one `Y.Text` whose formatting attributes are the marks. Inline atoms (mentions, emoji, math) are `Y.Text` embeds.
   - Moves are ID moves. This avoids y-prosemirror's whole-document replace and keeps remote edits O(changed blocks).
   - Put schema validation in Rust as content expressions compiled to an automaton, as ProseMirror does. Prefer `*` over `+` and **repair rather than delete** on invalid merges, per [CAVEATS.md](https://github.com/yjs/y-prosemirror/blob/master/CAVEATS.md).
   - Keep the in-memory render model (shaped lines, layout boxes) as a cache keyed by `(BlockId, version)` and derived from `yrs` observer events. It is never authoritative.
2. **Positions.**
   - The canonical local position is `(BlockId, offset)`, with the offset in **UTF-16 code units**. Set yrs `OffsetKind::Utf16` so offsets agree with Yjs JS peers, Android, iOS `NSString`, and Flutter. Convert to UTF-8 byte offsets only at the shaping boundary, using a per-block SumTree-style index (Zed) that carries byte, UTF-16, and grapheme metrics.
   - Use yrs **`StickyIndex`** (relative positions) for anything that outlives a transaction: remote cursors, comments, decorations, the composition anchor, bookmarks, and undo selections.
   - Offer ProseMirror-style integer positions only as a derived compatibility API, for TipTap-style extension ports, never as storage.
3. **CRDT mapping details.**
   - Represent comments and highlights that can overlap as **`StickyIndex` range annotations in a separate `Y.Map`**, not as formatting attributes. Same-type formatting marks cannot overlap.
   - Use `yrs::UndoManager` scoped by origin for collaborative undo.
   - Use awareness for carets (CollaborationCaret parity).
   - Use snapshots for version history and diffs (Snapshot and Snapshot Compare parity).
   - Watch Yjs v14 (`@y/y` RC) for schema and attribution support, and make sure documents stay readable by Yjs 13 peers until v14 is stable and yrs supports it.
4. **Input strategy per platform.** Use one Rust `InputSession` state machine: a surrounding-text window, selection, and composition range, taking platform deltas and producing editor ops plus IME geometry.
   - **Web:** EditContext on Chromium behind a runtime feature flag and a kill switch. Elsewhere, a hidden textarea mirroring the current block plus about 1k characters of context, driven by `beforeinput` and composition events and diff-reconciled. Never re-sync during composition.
   - **iOS:** a native `UIView` adopting `UITextInput` and `UITextInteraction`, through UniFFI.
   - **Android:** a custom `View` with a custom `InputConnection` that serves the text window, plus `updateSelection` and `updateCursorAnchorInfo` on every change and `restartInput` after remote edits near the caret.
   - **Flutter:** `DeltaTextInputClient` with `enableDeltaModel`.
   - **React Native:** a Fabric native component that wraps the iOS and Android views, not a WebView.
   - Build an **IME torture-test corpus** before features: Gboard, Samsung, SwiftKey, iOS dictation, Japanese, Korean, Chinese pinyin, and Vietnamese Telex.
5. **Shaping and rendering per platform.**
   - **Shape everywhere in Rust** with **parley 0.12** (harfrust + skrifa + ICU4X + fontique) for deterministic line breaks, identical across collaborators, and for hit-testing.
   - **Web:** fetch fonts as bytes. Render glyphs from skrifa outlines into a cached atlas (Canvas2D `drawImage` from an atlas canvas, with WebGL/WebGPU later), and use `fillText` per cluster only as a fallback or fast path.
   - **iOS:** `CTFontDrawGlyphs` with the same font bytes, registered through `CGFont`.
   - **Android:** `Canvas.drawGlyphs` (API 31 and later) with an atlas fallback.
   - **React Native:** draw through react-native-skia glyphs, or through the native views.
   - **Flutter:** render through a Rust-produced texture (or Impeller external texture) unless a glyph-run API exists. **This is the biggest unknown, so prototype it first.**
   - Ship a small default font set and treat emoji color fonts (COLRv1) as a separately loaded asset.
6. **Performance architecture.**
   - Virtualize by block, with layout only for the viewport ± 1 screen.
   - Keep per-block layout caches with incremental relayout of the edited paragraph only. Store height estimates for unlaid blocks in a SumTree for O(log n) scroll mapping.
   - Mark dirty rectangles, use one draw pass per frame, and do no per-keystroke whole-document passes in plugins. Give the plugin API incremental change sets: changed blocks plus ranges.
   - Target a CPU budget under 2 ms per keystroke at 1M characters, with no main-thread work over 4 ms.
7. **Bindings.**
   - Use UniFFI proc-macros as the single annotated surface, with uniffi-bindgen-react-native for RN and web where its wasm output suffices.
   - Use raw `wasm-bindgen` for the hot rendering and input path on the web.
   - Use flutter_rust_bridge v2 for Flutter.
   - Keep the FFI surface coarse: send ops and batches, and receive render diffs.
8. **Accessibility.**
   - The core emits a platform-neutral accessibility tree.
   - The web projects it into a mirror DOM, iOS into `UIAccessibilityReadingContent`, Android into an `AccessibilityNodeProvider`, and Flutter into `Semantics`.
   - Treat accessibility as an acceptance criterion for each node type in the parity checklist above.
9. **Benchmarks.** Publish a reproducible keypress-to-paint harness (Playwright + Chrome tracing) comparing canvist, ProseMirror/TipTap, Lexical, and Quill at 10k, 100k, and 1M characters, with p50, p95, and p99 results, and run it in CI.

**Generic-risk callout:** "Canvas TipTap" alone is a rewrite, not a twist. The differentiator worth leaning into is _deterministic, identical layout on every platform plus a collaborative core that is natively mobile_. No DOM editor can offer that.
