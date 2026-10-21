# Parity with TipTap and Remirror

This page compares canvist's extensions with every extension in [TipTap v3](https://github.com/ueberdosis/tiptap) (the open-source packages and the Pro ones) and [Remirror](https://github.com/remirror/remirror). Use it to check whether an editor you're moving from has something canvist doesn't, and what canvist calls it.

The comparison was made against TipTap `@tiptap/core` 3.30.3 and Remirror 3.0.3 (October 2026). Names in the **JSON** column are the ProseMirror node and mark names. canvist uses TipTap's names, so TipTap documents load as they are.

## How to read the status

| Status          | Meaning                                                                                                                                            |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Yes**         | An extension with the same JSON names, commands, keys, and typing rules, in Rust, TypeScript, and Dart. Differences are listed in the notes.       |
| **Built in**    | canvist does it without an extension, usually because the core already works that way.                                                             |
| **Host**        | It belongs to the app around the editor: a toolbar, a popup, or a drag-and-drop handler. canvist gives the host the queries and geometry it needs. |
| **Planned**     | Not shipped yet. The notes say what it will look like.                                                                                             |
| **Not planned** | canvist won't ship it. The notes say why and what to use instead.                                                                                  |

## Text formatting

| Feature           | TipTap               | Remirror                | JSON                        | canvist         | Notes                                                                                                                                                   |
| ----------------- | -------------------- | ----------------------- | --------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bold              | Bold                 | BoldExtension           | `bold`                      | **Yes**         | StarterKit.                                                                                                                                             |
| Italic            | Italic               | ItalicExtension         | `italic`                    | **Yes**         | StarterKit.                                                                                                                                             |
| Underline         | Underline            | UnderlineExtension      | `underline`                 | **Yes**         | StarterKit.                                                                                                                                             |
| Strike            | Strike               | StrikeExtension         | `strike`                    | **Yes**         | StarterKit. TipTap's `Mod-Shift-s` and `~~text~~`; Remirror uses `Mod-d` and a single `~`.                                                              |
| Inline code       | Code                 | CodeExtension           | `code`                      | **Yes**         | StarterKit. Excludes the marks it's registered with, where TipTap excludes every mark.                                                                  |
| Link              | Link                 | LinkExtension           | `link`                      | **Yes**         | StarterKit. Only `href`; TipTap's `target`, `rel`, `class`, and `title` attributes are planned.                                                         |
| Subscript         | Subscript            | SubExtension            | `subscript`                 | **Yes**         | `Mod-,`. Excludes superscript.                                                                                                                          |
| Superscript       | Superscript          | SupExtension            | `superscript`               | **Yes**         | `Mod-.`. Excludes subscript.                                                                                                                            |
| Highlight         | Highlight            | TextHighlightExtension  | `highlight`                 | **Yes**         | `==text==`, `Mod-Shift-h`. Multicolour by default, where TipTap defaults to one colour. Remirror calls it `textHighlight` with a `highlight` attribute. |
| Text style        | TextStyle            | —                       | `textStyle`                 | **Yes**         | The `<span style>` mark the next five extensions add attributes to.                                                                                     |
| Text colour       | Color                | TextColorExtension      | `textStyle.color`           | **Yes**         | Remirror uses its own `textColor` mark.                                                                                                                 |
| Background colour | BackgroundColor      | —                       | `textStyle.backgroundColor` | **Yes**         |                                                                                                                                                         |
| Font family       | FontFamily           | FontFamilyExtension     | `textStyle.fontFamily`      | **Yes**         | Remirror uses its own `fontFamily` mark.                                                                                                                |
| Font size         | FontSize             | FontSizeExtension       | `textStyle.fontSize`        | **Yes**         | Remirror's increase and decrease commands are planned.                                                                                                  |
| Line height       | LineHeight           | NodeFormattingExtension | `textStyle.lineHeight`      | **Yes**         | TipTap puts it on the text; Remirror puts it on the block.                                                                                              |
| Text alignment    | TextAlign            | NodeFormattingExtension | `textAlign` on blocks       | **Yes**         | `setTextAlign`, `toggleTextAlign`, `unsetTextAlign`, `Mod-Shift-l`/`e`/`r`/`j`. Layout aligns and justifies lines; HTML writes `text-align`.            |
| Indentation       | —                    | NodeFormattingExtension | `nodeIndent`                | **Planned**     | With text alignment.                                                                                                                                    |
| Text case         | —                    | TextCaseExtension       | `textCase`                  | **Planned**     | Needs `text-transform` in layout.                                                                                                                       |
| Ruby annotations  | RubyText             | —                       | `rubyText`                  | **Not planned** | Ruby needs a second line of text above the base; layout doesn't stack text yet.                                                                         |
| Clear formatting  | core `unsetAllMarks` | `ClearFormatting` key   | —                           | **Planned**     | A `Mod-Shift-c` command alongside text alignment.                                                                                                       |

## Blocks

| Feature             | TipTap                    | Remirror                | JSON                                            | canvist         | Notes                                                                                                        |
| ------------------- | ------------------------- | ----------------------- | ----------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------ |
| Document, paragraph | Document, Paragraph, Text | Doc, Paragraph, Text    | `doc`, `paragraph`                              | **Yes**         | Always registered.                                                                                           |
| Heading             | Heading                   | HeadingExtension        | `heading`                                       | **Yes**         | StarterKit. TipTap's `Mod-Alt-n`; Remirror's default map uses `Mod-Shift-n`.                                 |
| Blockquote          | Blockquote                | BlockquoteExtension     | `blockquote`                                    | **Yes**         | StarterKit.                                                                                                  |
| Code block          | CodeBlock                 | CodeBlockExtension      | `codeBlock`                                     | **Yes**         | StarterKit. Syntax highlighting is planned as a theme hook rather than a second extension.                   |
| Code block lowlight | CodeBlockLowlight         | CodeMirror 5 and 6      | `codeBlock`, `codeMirror`                       | **Not planned** | Embedding another editor inside a canvas editor defeats the point; highlighting will come through the theme. |
| Hard break          | HardBreak                 | HardBreakExtension      | `hardBreak`                                     | **Yes**         | StarterKit.                                                                                                  |
| Horizontal rule     | HorizontalRule            | HorizontalRuleExtension | `horizontalRule`                                | **Yes**         | StarterKit.                                                                                                  |
| Bullet list         | BulletList                | BulletListExtension     | `bulletList`                                    | **Yes**         | StarterKit.                                                                                                  |
| Ordered list        | OrderedList               | OrderedListExtension    | `orderedList`                                   | **Yes**         | StarterKit. TipTap's `type` attribute (`a`, `i`, …) is planned.                                              |
| List item           | ListItem, ListKeymap      | ListItemExtension       | `listItem`                                      | **Yes**         | StarterKit, with Enter, Tab, and Shift-Tab.                                                                  |
| Task list           | TaskList, TaskItem        | TaskListExtension       | `taskList`, `taskItem`                          | **Yes**         | StarterKit. Remirror calls the item `taskListItem`.                                                          |
| Image               | Image                     | ImageExtension          | `image`                                         | **Planned**     | Block images with `src`, `alt`, `title`, `width`, and `height`; the canvas reserves a slot the host fills.   |
| Audio               | Audio                     | —                       | `audio`                                         | **Planned**     | With image, as a slot.                                                                                       |
| Video embeds        | Youtube, Twitch           | IframeExtension         | `youtube`, `twitch`, `iframe`                   | **Planned**     | With image, as slots. A canvas can't host an iframe, so the host overlays the player.                        |
| File attachment     | FileHandler               | FileExtension           | `file`                                          | **Planned**     | A `file` card block. Reading dropped files stays with the host.                                              |
| Details             | Details                   | —                       | `details`, `detailsSummary`, `detailsContent`   | **Planned**     | Collapsing hides the content in layout too.                                                                  |
| Callout             | —                         | CalloutExtension        | `callout`                                       | **Planned**     | Remirror's `type` and `emoji`, with `:::info`.                                                               |
| Math                | Mathematics               | —                       | `inlineMath`, `blockMath`                       | **Planned**     | `$$x$$` and `$$$x$$$`, drawn by the host in a slot (KaTeX on the web).                                       |
| Tables              | TableKit                  | TableExtension          | `table`, `tableRow`, `tableCell`, `tableHeader` | **Planned**     | Merged cells, header rows and columns, Tab to move between cells, and a grid in layout.                      |
| Columns             | —                         | ColumnsExtension        | `columns`                                       | **Planned**     | After tables, sharing the grid layout.                                                                       |

## Inline nodes and suggestions

| Feature             | TipTap               | Remirror              | JSON                                  | canvist         | Notes                                                                                                                    |
| ------------------- | -------------------- | --------------------- | ------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Suggestion engine   | `@tiptap/suggestion` | `prosemirror-suggest` | —                                     | **Planned**     | One engine for every trigger: a query reports the active trigger, its text, and its range, and the host draws the popup. |
| Mention             | Mention              | MentionAtomExtension  | `mention`                             | **Planned**     | TipTap's atom with `id`, `label`, and `mentionSuggestionChar`. Remirror's mark form isn't planned.                       |
| Hashtag             | —                    | —                     | `hashtag`                             | **Planned**     | Neither has one; `#tag` becomes a mark as you type.                                                                      |
| Emoji               | Emoji                | EmojiExtension        | `emoji`                               | **Planned**     | TipTap's `name` attribute and `:shortcode:` rule. Remirror stores the character in `code`.                               |
| Variables and slots | Pro                  | —                     | `variable`, `blockSlot`, `inlineSlot` | **Not planned** | Template features for a later product, not the editor core.                                                              |

## Editing behaviour

| Feature              | TipTap               | Remirror              | canvist         | Notes                                                                                                                                                                 |
| -------------------- | -------------------- | --------------------- | --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| History              | UndoRedo             | HistoryExtension      | **Yes**         | StarterKit.                                                                                                                                                           |
| Trailing node        | TrailingNode         | TrailingNodeExtension | **Yes**         | StarterKit.                                                                                                                                                           |
| Typography           | Typography           | ShortcutsExtension    | **Yes**         | TipTap's 22 rules plus Remirror's `<->`. Rules ending in a digit (`2x3`, `^2`) fire on the next space or punctuation.                                                 |
| Character count      | CharacterCount       | CountExtension        | **Yes**         | `characters`, `words`, `characterLimit`, `setCharacterLimit`. Counts grapheme clusters, where TipTap counts UTF-16 code units.                                        |
| Placeholder          | Placeholder          | PlaceholderExtension  | **Yes**         | `showOnlyCurrent` and `includeChildren` as in TipTap. Layout draws it, so it looks the same on every platform.                                                        |
| Focus                | Focus                | —                     | **Planned**     | A query for the focused block path; the theme can style it.                                                                                                           |
| Invisible characters | InvisibleCharacters  | WhitespaceExtension   | **Planned**     | Drawn by layout: spaces, line ends, and breaks.                                                                                                                       |
| Unique ID            | UniqueID             | —                     | **Built in**    | Every block has a stable id, exported as `attrs.id` in JSON and `data-id` in HTML.                                                                                    |
| Gap cursor           | Gapcursor            | GapCursorExtension    | **Built in**    | Block selections cover rules, images, and other leaves, so there's always somewhere to put the caret.                                                                 |
| Drop cursor          | Dropcursor           | DropCursorExtension   | **Host**        | The host paints the drop line; `hit_test` and `block_rect` give it the position.                                                                                      |
| Selection            | Selection            | —                     | **Built in**    | The editor keeps its selection when it loses focus, and the host decides whether to paint it.                                                                         |
| Bidirectional text   | core `textDirection` | BidiExtension         | **Built in**    | Layout runs the Unicode bidi algorithm per block; `dir` on a block forces a direction.                                                                                |
| Node range selection | NodeRange            | —                     | **Built in**    | `Selection::Blocks` selects whole blocks.                                                                                                                             |
| Epic mode            | —                    | EpicModeExtension     | **Not planned** | Particles and shaking while typing. Fun, but a host effect; see [canvas-native ideas](#canvas-native-ideas) for the kind of visual canvist does in the editor itself. |

## Document tools

| Feature            | TipTap                   | Remirror                                      | canvist         | Notes                                                                                                               |
| ------------------ | ------------------------ | --------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------- |
| Find and replace   | FindAndReplace           | FindExtension                                 | **Planned**     | Queries for matches, commands to step and replace, and highlights drawn by layout.                                  |
| Table of contents  | TableOfContents          | —                                             | **Planned**     | A query that lists headings with their levels, numbers, and block ids.                                              |
| Comments           | Comments (Pro)           | AnnotationExtension, EntityReferenceExtension | **Planned**     | An overlapping `comment` mark holding thread ids. Threads themselves live outside the document.                     |
| Drag handle        | DragHandle               | —                                             | **Planned**     | Commands that move blocks, and the geometry for a handle. The handle itself is host UI.                             |
| Character limit    | CharacterCount `limit`   | CountExtension `maximum`                      | **Yes**         | Edits past the limit don't apply. A paste that overflows is refused whole, where TipTap trims it.                   |
| Tracked changes    | Tracked Changes (Pro)    | DiffExtension                                 | **Planned**     | After comments, on the same overlapping marks.                                                                      |
| Version history    | Snapshot (Pro)           | —                                             | **Built in**    | `Doc::snapshot` captures a version and `Engine::restore` brings it back; named versions in canvist Cloud come next. |
| Version compare    | Snapshot Compare (Pro)   | DiffExtension                                 | **Planned**     | With tracked changes.                                                                                               |
| Pages              | Pages (Pro)              | —                                             | **Planned**     | Pagination is a natural fit for canvas layout; see [canvas-native ideas](#canvas-native-ideas).                     |
| Markdown           | `@tiptap/markdown`       | MarkdownExtension                             | **Built in**    | Import and export in every binding, with GitHub task lists and strikethrough.                                       |
| Import and export  | Import, Export (Pro)     | —                                             | **Built in**    | JSON, HTML, Markdown, and text. DOCX, ODT, and PDF are planned for canvist Cloud's convert API.                     |
| Paste cleanup      | Paste Handler (Pro)      | —                                             | **Built in**    | The HTML reader keeps structure and formatting from Word and Google Docs and drops the rest.                        |
| Content protection | Content Protection (Pro) | —                                             | **Not planned** | A policy layer for template products, not the editor core.                                                          |

## Collaboration

| Feature         | TipTap             | Remirror                     | canvist         | Notes                                                                                     |
| --------------- | ------------------ | ---------------------------- | --------------- | ----------------------------------------------------------------------------------------- |
| Yjs sync        | Collaboration      | YjsExtension                 | **Built in**    | `canvist_collab` is a plain Yjs peer: y-websocket, Hocuspocus, and JavaScript peers work. |
| Remote carets   | CollaborationCaret | YjsExtension `cursorBuilder` | **Built in**    | Awareness presence with carets and selections.                                            |
| Step sync       | —                  | CollaborationExtension       | **Not planned** | Yjs covers it.                                                                            |
| Hosted provider | Provider (Pro)     | —                            | **Built in**    | canvist Cloud.                                                                            |

## Host UI

These are the app's job in a canvas editor. canvist provides what each one needs.

| Feature       | TipTap       | Remirror                | What canvist provides                                                                       |
| ------------- | ------------ | ----------------------- | ------------------------------------------------------------------------------------------- |
| Bubble menu   | BubbleMenu   | PositionerExtension     | `selection_bounds` and typed queries such as `isBold`.                                      |
| Floating menu | FloatingMenu | PositionerExtension     | `caret_rect` and `block_rect`.                                                              |
| Node views    | node views   | ReactComponentExtension | Slots in the display list for the host to draw into.                                        |
| DOM events    | core events  | EventsExtension         | Hosts forward keys, text, and pointer input; `hit_test` maps points to positions.           |
| File drop     | FileHandler  | file paste rules        | `hit_test` for the drop position, and insert commands for images and files.                 |
| AI writing    | AI (Pro)     | —                       | Headless `Doc` editing and typed commands for agents; see [AI agents](../agents/readme.md). |

## Canvas-native ideas

A canvas editor draws every glyph itself, so it can do things a DOM editor can't do cheaply. None of these exist in TipTap or Remirror. They're listed roughly from most to least useful.

| Idea              | What it does                                                                                                                            | Why canvas                                                                                                                                                         |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Ink**           | Pen and finger strokes drawn over the text and anchored to it, so a circled word stays circled when the paragraph reflows.              | Strokes live in the same coordinate space as glyphs, and layout knows where every anchored character moved. A DOM editor has to overlay a second canvas and guess. |
| **Kinetic type**  | A mark that animates its text: a wave, a pulse, a shiver. The document stays plain text with a mark; the painter moves the glyphs.      | Per-glyph transforms every frame are a few multiplications for a canvas and thousands of DOM nodes for anyone else.                                                |
| **Redaction**     | A mark that paints solid bars over text and leaves it out of copy, export, and the accessibility mirror for readers without permission. | The canvas never puts the text on screen, so there's nothing to select or inspect. In a DOM editor, redacted text is still in the page.                            |
| **Pages**         | Real pagination with headers and footers, measured once and identical in the browser, on the server, and in print.                      | Layout already measures every line deterministically, so page breaks are just another query.                                                                       |
| **Focus reading** | Dims everything but the current sentence or paragraph, with a smooth fade as the caret moves.                                           | The painter blends colours per run; no restyling or reflow.                                                                                                        |
| **Minimap**       | A zoomed-out overview of the whole document beside the editor.                                                                          | The display list draws at any scale, so the minimap is the same paint call with a transform.                                                                       |
| **Margin notes**  | Notes laid out in the margin next to the text they belong to, pushed apart so they never overlap.                                       | Layout knows the exact position of every anchor, so notes can be placed in the same pass.                                                                          |

canvist builds **Ink** and **Kinetic type** first. They change how people use an editor (circling and underlining by hand, text that moves to make a point), and both fall out of what layout already knows.
