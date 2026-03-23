---
"canvist_core": patch
---

Fix TypeScript type checking — 586 errors → 0

### Root cause

The `WasmCanvistEditor` interface in `wasm.ts` only declared ~30 of the 568+
WASM methods. The `editor.ts` wrapper used all 568, causing 586 type errors
that were masked by `--no-check` builds.

### Fix

- `WasmCanvistEditor` now re-exports the auto-generated type from
  `wasm/canvist_wasm.d.ts` via `import type`, ensuring all WASM methods are
  available with correct signatures.
- Added `diff_texts` static method to `CanvistWasmModule` interface.
- Fixed `colorRgba` argument type: `number[]` → `Uint8Array`.
- Added missing `setTextAlign`, `textAlign`, `enableCollab`, `collabEnabled`,
  `collabEncodeState/Vector`, `collabApplyUpdate`, `collabSyncLocal` to the
  public editor API object.

`deno check src/editor.ts` now passes with **zero errors**.
