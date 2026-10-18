---
"canvist_collab": minor
---

Add syncing, presence, and a swappable history trait to `canvist_collab`.

- `SyncSession` speaks the y-sync protocol (sync step 1 and 2, updates, awareness) over any transport, framed for y-websocket servers or for Hocuspocus, including its authentication step.
- `Presence` publishes the local user and selection through Yjs awareness as relative positions, in y-prosemirror's state shape, and resolves other peers' carets into `Collaborator`s.
- `History` is the trait an editor uses for undo; `Binding` implements it with a yrs `UndoManager` that only undoes local changes.
- Tests sync through a yrs relay server and through a real Hocuspocus server with a JavaScript `HocuspocusProvider`, carets included.
