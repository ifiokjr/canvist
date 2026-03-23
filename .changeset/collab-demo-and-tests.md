---
"canvist_core": patch
---

Add collaboration demo, Ctrl+S save, and convergence tests

### Collaboration demo

New `demo/collab.html` page demonstrates real-time collaborative editing
between browser tabs using the Yrs CRDT + BroadcastChannel (no server needed).
Each tab gets a random color cursor visible to other tabs. Edits sync
automatically with debouncing.

### Ctrl+S save

The main demo now handles Ctrl+S to save immediately to localStorage with a
visual status bar flash and screen reader announcement.

### New tests

5 new tests:
- **Delete across multiple paragraph boundaries** — removes 2 paragraphs at once
- **Insert at paragraph boundary** — inserts text at the start of a paragraph
- **Empty paragraphs from consecutive newlines** — verifies `A\n\nB` creates 3
  paragraphs
- **Concurrent edit convergence** — 2 peers make concurrent edits, both converge
- **Three-peer convergence** — 3 peers with concurrent edits all converge
