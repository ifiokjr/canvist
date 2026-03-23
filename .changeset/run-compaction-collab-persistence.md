---
"canvist_core": minor
---

Add run compaction, collaboration wiring, and demo persistence

### Run compaction

`Document::compact_runs()` is now called automatically during
`rebuild_indexes()`. Adjacent text runs with identical styles are merged into
a single run, preventing fragmentation from repeated format/undo cycles. Empty
runs are also removed (except the last run in a paragraph, which is kept to
preserve document structure for insert operations).

### Collaboration WASM API

The `CanvistEditor` now exposes a complete collaboration API to JavaScript:
- `enable_collab()` — creates a Yrs CRDT session and syncs the document
- `collab_encode_state()` — full binary state for bootstrapping peers
- `collab_encode_state_vector()` — state vector for incremental sync
- `collab_apply_update(update)` — apply a remote peer's binary update
- `collab_sync_local()` — push local edits into the CRDT

### Demo persistence

The demo now saves/restores document content and cursor position via
`localStorage`. Content auto-saves every 2 seconds and on page unload.
