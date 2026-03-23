---
"canvist_core": patch
---

Fix TypeScript editor wrapper and update types

### TypeScript editor fixes

Port the same mouse interaction fixes from the demo HTML to the TypeScript
editor wrapper (`packages/canvist/src/editor.ts`):

- **Double-click selection persistence** via `skipNextClick` flag
- **Triple-click-and-drag** line-level selection with `selectionMode` tracking
- **Double-click-and-drag** word-level selection with word boundary snapping
- Added `wordAnchorStart/End` and `lineAnchorStart/End` for drag anchors

### Updated TypeScript types

Added type definitions for new WASM APIs:
- `setTextAlign(align)` / `textAlign` — text alignment control
- `enableCollab()` / `collabEnabled` — collaboration session lifecycle
- `collabEncodeState()` / `collabEncodeStateVector()` — state export
- `collabApplyUpdate(update)` — remote update ingestion
- `collabSyncLocal()` — push local edits to CRDT
