---
"@canvist/web": feat
canvist_web: feat
canvist_headless: feat
canvist_editor: feat
---

# Collaborate over y-websocket out of the box with `@canvist/web/collab`

`@canvist/web/collab` joins a browser editor to a room on any server that speaks y-websocket's protocol: y-websocket's own server, canvist's `receiveSync` servers, or canvist Cloud Sync. The editor is the Yjs peer itself, so there is no `Y.Doc` to keep in step, and JavaScript Yjs peers in the room see the same document.

```ts
import { connect, createEditor } from "@canvist/web/collab";

const editor = await createEditor(element);
const connection = connect(editor, {
	url: "wss://sync.example.com",
	room: "notes",
});
await connection.whenSynced();
```

- `createEditor` from `@canvist/web/collab` loads a second WebAssembly module (`wasm-collab/`) with Yjs inside, so editors that don't collaborate never download it. `loadCollab(source)` loads it from elsewhere.
- `connect(editor, { url, room, params, clientId, reconnect, WebSocket })` returns a `Connection` with `status`, `synced`, `clientId`, `subscribe`, `whenSynced`, and `disconnect`. It reconnects with backoff and catches up both ways.
- Undo is per person: `Mod-z`, `editor.undo()`, and the `undo` command revert only this peer's changes, grouped by word as when editing alone.
- `canvist_headless`: `join_message()` and `Engine::join_sync` join a shared document over y-sync with no state of its own; `Doc::receive_sync_changes` also returns the commit of each remote change; `Doc::history_step` undoes or redoes and returns the update; `Doc::shared_history_key` and `Doc::shared_history_command` identify History's keys and commands on a shared document, which `press` and `run_named` now answer with the shared history. `Doc::edit` joins any follow-up the shared state needed to the update it returns, and integrating a collaborator's change no longer advances the document's clock (y-websocket echoes each update to its sender, which split typing into one undo step per key).
- `canvist_editor`: `UndoGrouping` is the rule `UndoHistory` groups changes by, on its own, so a shared history can group the same way.
