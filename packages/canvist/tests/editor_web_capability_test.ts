import { assertEquals, assertRejects } from "@std/assert";
import { createEditor } from "../src/editor.ts";

const HAS_DOM = typeof document !== "undefined";

Deno.test({
	name: "web api: text insert/delete, keyboard paths, and IME composition",
	ignore: !HAS_DOM,
	fn: async () => {
		const root = document.createElement("div");
		document.body.appendChild(root);
		root.innerHTML =
			`<canvas id="editor-main" width="640" height="240"></canvas>`;

		const editor = await createEditor("editor-main", { title: "Parity" });
		const canvas = document.getElementById("editor-main") as HTMLCanvasElement;
		const textarea = document.getElementById(
			"canvist-input",
		) as HTMLTextAreaElement;

		// Direct API insertion.
		editor.insertText("Hello");
		assertEquals(editor.text, "Hello");
		assertEquals(editor.charCount, 5);

		// Keyboard path: Enter and printable character insertion via input event.
		textarea.dispatchEvent(
			new KeyboardEvent("keydown", {
				key: "Enter",
				bubbles: true,
				cancelable: true,
			}),
		);
		textarea.dispatchEvent(
			new InputEvent("input", { data: "W", bubbles: true }),
		);
		assertEquals(editor.text, "Hello\nW");

		// Keyboard path: ArrowLeft then Backspace removes previous char.
		textarea.dispatchEvent(
			new KeyboardEvent("keydown", {
				key: "ArrowLeft",
				bubbles: true,
				cancelable: true,
			}),
		);
		textarea.dispatchEvent(
			new KeyboardEvent("keydown", {
				key: "Backspace",
				bubbles: true,
				cancelable: true,
			}),
		);
		assertEquals(editor.text, "HelloW");

		// Keyboard path: ArrowLeft then Delete removes next char.
		textarea.dispatchEvent(
			new KeyboardEvent("keydown", {
				key: "ArrowLeft",
				bubbles: true,
				cancelable: true,
			}),
		);
		textarea.dispatchEvent(
			new KeyboardEvent("keydown", {
				key: "Delete",
				bubbles: true,
				cancelable: true,
			}),
		);
		assertEquals(editor.text, "Hello");

		// IME path via compositionend.
		textarea.dispatchEvent(
			new CompositionEvent("compositionend", { data: "世界", bubbles: true }),
		);
		assertEquals(editor.text, "Hello世界");
		assertEquals(editor.charCount, 7);

		// Rendering smoke assertion should not throw.
		editor.render();
		assertEquals(canvas.getAttribute("role"), "textbox");
		assertEquals(canvas.getAttribute("aria-multiline"), "true");
		assertEquals(canvas.getAttribute("aria-valuetext"), editor.text);

		editor.destroy();
		root.remove();
	},
});

Deno.test({
	name: "web api: serialization roundtrip with metadata/title",
	ignore: !HAS_DOM,
	fn: async () => {
		const root = document.createElement("div");
		document.body.appendChild(root);
		root.innerHTML =
			`<canvas id="editor-json" width="640" height="240"></canvas>`;

		const editor = await createEditor("editor-json");
		editor.setTitle("Roundtrip Title");
		editor.insertText("Line 1\nLine 2");

		const json = editor.toJSON();
		const parsed = JSON.parse(json) as {
			title?: string;
			nodes?: Array<{ text?: string }>;
		};

		assertEquals(parsed.title, "Roundtrip Title");
		const aggregated = (parsed.nodes ?? []).map((n) => n.text ?? "").join("");
		assertEquals(aggregated.includes("Line 1"), true);
		assertEquals(aggregated.includes("Line 2"), true);

		editor.destroy();
		root.remove();
	},
});

Deno.test("web api: fails fast for missing canvas", async () => {
	await assertRejects(() => createEditor("does-not-exist"));
});
