/**
 * A collaborative editor for the collaboration tests, joined to
 * `?room=` on the y-websocket server at `?server=`. `window.editor` is
 * the editor and `window.connection` its connection, once the room's
 * document has arrived; `window.reconnect()` connects it again, and
 * `window.modifier` is the key `Mod` means.
 */
import { connect, createEditor } from "../../src/collab.ts";
import { detectPlatform } from "../../src/mod.ts";

const params = new URLSearchParams(location.search);
const editor = await createEditor(
	document.querySelector<HTMLElement>("#host")!,
	{
		label: "Shared editor",
		placeholder: "Nobody has written anything yet",
	},
);
const options = {
	url: params.get("server") ?? "ws://127.0.0.1:5181",
	room: params.get("room") ?? "default",
};
const connection = connect(editor, options);
await connection.whenSynced();

/** Connect again after `connection.disconnect()`. */
async function reconnect(): Promise<void> {
	const again = connect(editor, options);
	await again.whenSynced();
	Object.assign(globalThis, { connection: again });
}

Object.assign(globalThis, {
	editor,
	connection,
	reconnect,
	modifier: detectPlatform(navigator) === "apple" ? "Meta" : "Control",
});
