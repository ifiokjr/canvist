# canvist_collab

Real-time collaboration for [canvist](https://github.com/ifiokjr/canvist) documents over Yjs, using [`yrs`](https://crates.io/crates/yrs).

- **Binding:** mirrors a `canvist_model` document into a yrs document and back. Each model step becomes yrs operations and each remote yrs event becomes steps, so nothing is ever diffed.
- **Plain Yjs:** the shared document follows a documented schema of `Y.Map` blocks, `Y.Text` with marks as formatting, and nested `Y.Array`s, so JavaScript Yjs peers, y-websocket, and Hocuspocus take part without knowing about canvist.
- **Robust:** updates may arrive out of order or twice, blocks that break the schema are hidden rather than deleted, and concurrent moves of one block keep a single copy.
- **Shared undo:** a yrs `UndoManager` that only undoes local changes.

Mirroring a keystroke into a Yjs update takes about 2 µs, and applying a remote keystroke about 2.5 µs, whether the document has 100 blocks or 10,000. Run `cargo bench -p canvist_collab` to reproduce.

The compatibility tests run a real JavaScript Yjs peer under [Deno](https://deno.com), which the devenv shell provides.
