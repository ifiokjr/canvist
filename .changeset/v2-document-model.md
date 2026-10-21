---
"canvist_model": minor
---

# Add `canvist_model`, the v2 block-based document model

- `Schema` built from block, mark, and embed specs that extensions contribute.
- `Document` with stable `BlockId`s, nested containers, and `InlineText` that stores formatting as mark runs and embeds as single characters.
- Invertible `Step`s with point mapping. `Transaction` applies them atomically with rollback, and `Commit` records which blocks changed for incremental layout.
- ProseMirror/TipTap-compatible JSON import and export.
- Property tests for invariants and exact undo, a 20,000-keystroke typing stress test, and `divan` benchmarks covering 100 to 100,000 blocks.
