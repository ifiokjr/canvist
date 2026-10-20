---
canvist-dart: feat
---

# Add editor views, server rendering, and key platforms to the Dart package

- `doc.view(measurer)` returns an `EditorView`: incremental layout of the document, a display list per frame (`frame`, which returns no display when nothing changed), keys, typing, IME composition, the text an input method edits, pointer and handle selection, clipboard formats, typed commands with wall-clock undo grouping, caret and selection geometry, and accessibility text. It shares the document, so commands run on the `Doc` and collaborators' updates reach the layout too.
- `TextMeasurer` is the host's half of layout: Rust calls it synchronously, once per relayout batch, through a `NativeCallable`. `FixedMeasurer` gives predictable layout for tests and servers. A measurer that throws fails the view call that needed it.
- `doc.toHtml(width)`, `doc.toSvg(width)`, and `doc.serverDisplay(width)` render on the server, as the editor lays out.
- `EngineBuilder.build` and `Engine.starterKit` take a `KeyPlatform`, so `Mod` is Cmd on Apple devices.
- Budgets for a keystroke through a view to its next frame (250 µs at 10,000 paragraphs, measured at about 65 µs) and an unchanged frame.
