---
"canvist_ffi": minor
---

# A native rich-text editor for Android

`canvist-android` now ships `CanvistView`, a rich-text editor over the canvist engine. It paints on a `Canvas`, measures with `Paint`, and takes text through its own `InputConnection`, so Gboard composition, autocorrect, suggestions, and hardware keyboard shortcuts work as they do in an `EditText`. It also has selection handles with a magnifier, a floating text toolbar with formatting, TalkBack support through an `AccessibilityNodeProvider`, and scrolling with fling. Typed handles for every extension come from `StarterKit` and `Extensions`:

```kotlin
val kit = StarterKit()
val view = CanvistView(context, kit.extensions)
view.editor.load(DocFormat.MARKDOWN, "# Hello")
view.editor.run(kit.bold.toggleBold())
```

The sample app is now a full editor with a formatting toolbar and two editors collaborating.
