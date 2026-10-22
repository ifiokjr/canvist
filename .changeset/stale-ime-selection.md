---
canvist_flutter: fix
---

# Keep a selection the app sets while the keyboard is still starting

A keyboard's first updates after the editor gains focus describe the caret the editor opened with. When an app set a new selection in that moment, for example selecting everything as the editor opens, those late updates arrived after it and collapsed the selection, so the next format applied to nothing. `CanvistEditor` now recognises an update that only returns to a selection it has since replaced, and keeps its own.
