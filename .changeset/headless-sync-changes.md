---
"canvist_headless": feat
---

# Return the commits a y-sync message applied

`Doc::receive_sync_changes` handles a y-sync message as `receive_sync` does and also returns the commit of every update that changed the document, so interactive hosts lay out only what changed.
