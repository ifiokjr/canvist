/**
 * Collaboration through y-websocket's own server: editors in one room see
 * each other's edits, a JavaScript Yjs peer shares their document, undo
 * reverts only the peer's own changes, and a peer that drops off catches up.
 *
 * @module
 */
import { createEngine } from "@canvist/headless";
import { expect, type Page, test } from "@playwright/test";
import { WebsocketProvider } from "y-websocket";
import * as Y from "yjs";
import type { Connection } from "../../src/collab.ts";
import type { Editor, StarterKit } from "../../src/mod.ts";

declare global {
	var editor: Editor<StarterKit>;
	var connection: Connection;
	var modifier: "Meta" | "Control";
	var reconnect: () => Promise<void>;
}

/** The y-websocket server `playwright.config.ts` starts. */
const SERVER = "ws://127.0.0.1:5181";

/** A room no other test uses. */
function room(name: string): string {
	return `${name}-${crypto.randomUUID()}`;
}

async function join(page: Page, name: string): Promise<void> {
	await page.goto(`/collab.html?${new URLSearchParams({ room: name })}`);
	await page.waitForFunction(() => globalThis.connection?.synced === true);
}

function markdown(page: Page): Promise<string> {
	return page.evaluate(() => editor.export("markdown").trim());
}

/** Type `text` at the end of the first block. */
async function typeAtEnd(page: Page, text: string): Promise<void> {
	await page.evaluate(() => {
		editor.select({ path: [0], offset: 0 });
		editor.select({ path: [0], offset: editor.blockText()?.length ?? 0 });
		editor.focus();
	});
	await page.keyboard.type(text);
}

test.describe("collaboration", () => {
	test("editors in one room see each other's edits", async ({ browser }) => {
		const name = room("typing");
		const alice = await browser.newPage();
		const bob = await browser.newPage();
		await join(alice, name);
		await join(bob, name);

		expect(await alice.evaluate(() => editor.isEmpty), "an empty room")
			.toBe(true);
		await typeAtEnd(alice, "Hello");
		await expect.poll(() => markdown(bob)).toBe("Hello");

		await typeAtEnd(bob, " world");
		await expect.poll(() => markdown(alice)).toBe("Hello world");
		expect(await bob.evaluate(() => editor.export("json"))).toBe(
			await alice.evaluate(() => editor.export("json")),
		);
	});

	test("a JavaScript Yjs peer shares the document", async ({ page }) => {
		const name = room("yjs");
		const ydoc = new Y.Doc();
		const provider = new WebsocketProvider(SERVER, name, ydoc, {
			WebSocketPolyfill: WebSocket,
		});
		await new Promise<void>((resolve) =>
			provider.once("sync", () => resolve())
		);

		try {
			await join(page, name);
			await typeAtEnd(page, "From the canvas");

			// The Yjs peer holds canvist's document.
			const engine = await createEngine();
			await expect.poll(() =>
				engine.fromYjs(Y.encodeStateAsUpdate(ydoc), 77).export("markdown")
					.trim()
			).toBe("From the canvas");

			// And its edits reach the canvas.
			const peer = engine.fromYjs(Y.encodeStateAsUpdate(ydoc), 78);
			peer.select({ path: [0], offset: 0 });
			peer.insertText("Yjs: ");

			for (const update of peer.takeUpdates()) {
				Y.applyUpdate(ydoc, update);
			}

			await expect.poll(() => markdown(page)).toBe("Yjs: From the canvas");
		} finally {
			provider.destroy();
			ydoc.destroy();
		}
	});

	test("undo reverts only this peer's changes", async ({ browser }) => {
		const name = room("undo");
		const alice = await browser.newPage();
		const bob = await browser.newPage();
		await join(alice, name);
		await join(bob, name);

		await typeAtEnd(alice, "Alice");
		await expect.poll(() => markdown(bob)).toBe("Alice");
		await typeAtEnd(bob, " and Bob");
		await expect.poll(() => markdown(alice)).toBe("Alice and Bob");

		const modifier = await alice.evaluate(() => globalThis.modifier);
		await alice.keyboard.press(`${modifier}+z`);
		await expect.poll(() => markdown(alice)).toBe("and Bob");
		await expect.poll(() => markdown(bob)).toBe("and Bob");

		await alice.evaluate(() => editor.redo());
		await expect.poll(() => markdown(bob)).toBe("Alice and Bob");
	});

	test("a peer that drops off catches up when it reconnects", async ({ browser }) => {
		const name = room("reconnect");
		const alice = await browser.newPage();
		const bob = await browser.newPage();
		await join(alice, name);
		await join(bob, name);
		await typeAtEnd(alice, "Shared");
		await expect.poll(() => markdown(bob)).toBe("Shared");

		await alice.evaluate(() => connection.disconnect());
		expect(await alice.evaluate(() => connection.status)).toBe(
			"disconnected",
		);
		await typeAtEnd(alice, " offline");
		await typeAtEnd(bob, " online");
		await expect.poll(() => markdown(bob)).toBe("Shared online");

		await alice.evaluate(() => globalThis.reconnect());

		await expect.poll(() => markdown(alice)).toMatch(
			/^Shared( offline| online){2}$/,
		);
		await expect.poll(async () => markdown(bob)).toBe(await markdown(alice));
	});
});
