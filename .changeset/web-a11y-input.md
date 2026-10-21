---
"@canvist/web": fix
---

# Screen readers meet the text field first, and placeholders have 4.5:1 contrast

- In Firefox and Safari, where text arrives through a hidden textarea, the textarea now sits right after the canvas, as EditContext's field does in Chromium, so screen readers reach the editor's text field before its document mirror.
- The placeholder shows the body text's colour at 62% instead of 50%, which gives 4.5:1 contrast on the StarterKit's light and dark pages.
- New browser tests: an axe audit (WCAG 2.2 AA) of editable, read-only, empty, and menu-showing editors; virtual screen reader smoke tests; and Safari composition, Android keyboard, and touch input sequences in Chromium, Firefox, and WebKit.
