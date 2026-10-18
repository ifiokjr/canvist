# canvist_editor

The platform-agnostic editing engine behind [canvist](https://github.com/ifiokjr/canvist), built on [`canvist_model`](../canvist_model).

- **Editor state:** the document, selection, stored marks, and IME composition, changed through one path so they never disagree.
- **Commands:** typing with mark inclusivity, deleting by grapheme, word, line, or document, Enter and Backspace that understand lists and quotes, marks, block types, wrapping and lifting, and copy and paste.
- **Selection motion:** by grapheme cluster and word (UAX #29), line, and document, with inline embeds and atomic blocks treated as single stops. Vertical motion asks a `LineNavigator`, which the layout engine implements.
- **Editor:** history with grouping by time and kind of edit and selection restore (behind a `History` trait the collaboration crate replaces), keymaps where `Mod` is Cmd on Apple platforms and Ctrl elsewhere, input rules such as `##` to a heading, and paste rules, all contributed by `Extension`s so an app pays only for what it registers.
- **Headless, typed API:** registering an extension returns a typed handle whose methods are the only typed way to reach its commands, queries, and attributes. Named commands and queries with checked arguments serve dynamic hosts, and machine-readable metadata (`Editor::metadata_json`) feeds binding generators. `can()` dry runs and `chain()` all-or-nothing chains change nothing unless they apply, down to block revisions.
