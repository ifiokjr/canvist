---
canvist_flutter: fix
---

# Stop restarting Android keyboards while the editor has focus

Android keyboards mark the word at the caret when their input starts, for suggestions. `CanvistEditor` sent its own state back without that mark, which made Android restart the keyboard, which marked the word again, several times a second for as long as the editor had focus. The keyboard and its suggestion strip churned, and suggestions and autocorrect kept resetting. The editor now leaves the word the keyboard marks alone, and composing over it works as before.
