---
canvist_headless: feat
---

# Record every commit for hosts that lay a document out

`Doc::record_commits` turns on a journal of every commit that changes the document: local calls, collaborators' updates, the shared undo history, and the repairs collaboration makes. `Doc::take_commits` drains it. A host keeping a layout applies each commit, so changes made outside its own `Doc::edit` calls, such as commands other code runs or updates from peers, still reach the layout.

`Renderer::display_list` returns the display list `Renderer::svg` paints, so native hosts can compare their own rendering with server rendering.
