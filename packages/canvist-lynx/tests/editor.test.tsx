/**
 * The JavaScript half of `@canvist/lynx`: the attributes `<canvist-editor>`
 * gets, the typed editor over the `CanvistEditors` module, and the events
 * that drive subscribers. The native module is a fake that records what
 * reaches it.
 */
import "@testing-library/jest-dom";
import { act, render } from "@lynx-js/react/testing-library";
import { beforeEach, expect, rstest, test } from "@rstest/core";

import {
	bold,
	CanvistEditor,
	CanvistView,
	type Editor,
	heading,
	isCanvistError,
	useCanvistEditor,
	useEditorState,
} from "../src/mod.ts";
import { answers, calls, failures, fakeModule } from "./fakes.ts";

type Kit = [typeof bold, typeof heading];

beforeEach(() => {
	globalThis.NativeModules = {
		...globalThis.NativeModules,
		CanvistEditors: fakeModule,
	};
	calls.length = 0;
	answers.clear();
});

/** The testing library's two-thread environment, which its package types leave unexported. */
declare const lynxTestingEnv: {
	switchToBackgroundThread(): void;
	switchToMainThread(): void;
};

/**
 * Fire the element's event `name` with `detail` on the background thread,
 * where event handlers run, inside `act`, as the testing library's
 * `fireEvent` does (it accepts only built-in elements). The background
 * thread has its own copy of the element.
 */
function emit(element: Element, name: string, detail: object): void {
	const selector = `[editor-id="${element.getAttribute("editor-id")}"]`;
	lynxTestingEnv.switchToBackgroundThread();

	try {
		const event = new Event(`bindEvent:${name}`);
		Object.assign(event, { eventType: "bindEvent", eventName: name, detail });
		act(() => {
			document.querySelector(selector)?.dispatchEvent(event);
		});
	} finally {
		lynxTestingEnv.switchToMainThread();
	}
}

function editorElement(container: Element): Element {
	const element = container.querySelector("canvist-editor");

	if (element === null) {
		throw new Error("no <canvist-editor> rendered");
	}

	return element;
}

/** Render an editor of bold and heading, and make it ready. */
function readyEditor(props: { onUpdate?: (editor: Editor<Kit>) => void } = {}) {
	let ready: Editor<Kit> | null = null;
	const { container } = render(
		<CanvistEditor
			extensions={[bold, heading]}
			onReady={(editor) => {
				ready = editor;
			}}
			{...props}
		/>,
	);
	const element = editorElement(container);
	emit(element, "ready", { editorId: element.getAttribute("editor-id") });

	if (ready === null) {
		throw new Error("the editor never became ready");
	}

	return { editor: ready as Editor<Kit>, element };
}

test("the element gets its extensions and starting document", () => {
	const { container } = render(
		<CanvistEditor
			extensions={[bold, heading]}
			content={{ format: "markdown", input: "# Notes" }}
			textSize={19}
			accessibility-label="Note"
		/>,
	);
	const element = editorElement(container);

	expect(element.getAttribute("editor-id")).toMatch(/^canvist-\d+$/);
	expect(element.getAttribute("content-format")).toBe("markdown");
	expect(element.getAttribute("content")).toBe("# Notes");
	expect(element.getAttribute("text-size")).toBe("19");
	expect(element.getAttribute("accessibility-label")).toBe("Note");
});

test("typed commands and queries reach the editor the element registered", () => {
	answers.set("headingLevel", "2");
	const { editor, element } = readyEditor();
	const id = element.getAttribute("editor-id");

	expect(editor.commands.toggleHeading({ level: 2 })).toBe(true);
	expect(editor.query.headingLevel()).toBe(2);
	expect(editor.can.toggleBold()).toBe(false);
	expect(editor.export("markdown")).toBe("Hello");
	expect(editor.selection).toEqual({
		anchor: { path: [0], offset: 1 },
		head: { path: [0], offset: 3 },
	});
	editor.setContent({ format: "markdown", input: "**Hi**" });
	expect(editor.undo()).toBe(true);

	expect(calls).toEqual([
		`run ${id} toggleHeading {"level":2}`,
		`query ${id} headingLevel null`,
		`can ${id} toggleBold null`,
		`export ${id} markdown`,
		`selection ${id}`,
		`load ${id} markdown **Hi**`,
		`undo ${id}`,
	]);
});

test("change and focus events reach onUpdate and the editor's state", () => {
	const onUpdate = rstest.fn();
	const { editor, element } = readyEditor({ onUpdate });

	emit(element, "change", {
		document: true,
		selection: false,
		composition: false,
	});
	expect(onUpdate).toHaveBeenCalledTimes(1);

	emit(element, "focuschange", { focused: true });
	expect(editor.focused).toBe(true);
	expect(onUpdate).toHaveBeenCalledTimes(2);
});

test("native failures arrive as canvist errors with their codes", () => {
	const { editor } = readyEditor();
	failures.run = "[canvist:invalidArgs] level must be 1 to 6";

	let caught: unknown;
	try {
		editor.commands.toggleBold();
	} catch (error) {
		caught = error;
	}

	expect(isCanvistError(caught)).toBe(true);
	expect(caught).toMatchObject({
		code: "invalidArgs",
		message: "level must be 1 to 6",
	});
});

test("an editor the element could not create reports through onError", () => {
	const onError = rstest.fn();
	const { container } = render(<CanvistEditor onError={onError} />);

	emit(editorElement(container), "error", {
		code: "config",
		message: "unknown extension",
	});

	expect(onError).toHaveBeenCalledWith(
		expect.objectContaining({ code: "config", message: "unknown extension" }),
	);
});

test("a missing native module is a config error, not a crash", () => {
	const { CanvistEditors: _registered, ...unregistered } =
		globalThis.NativeModules;
	globalThis.NativeModules = unregistered;
	const onError = rstest.fn();
	const { container } = render(<CanvistEditor onError={onError} />);
	const element = editorElement(container);

	emit(element, "ready", { editorId: element.getAttribute("editor-id") });

	expect(onError).toHaveBeenCalledWith(
		expect.objectContaining({ code: "config" }),
	);
});

test("useEditorState re-renders with the editor's state after each change", () => {
	function Toolbar() {
		const { editor, props } = useCanvistEditor({ extensions: [bold] });
		const active = useEditorState(editor, (e) => e.query.isBold(), false);

		return (
			<view>
				<text>{active ? "bold" : "plain"}</text>
				<CanvistView {...props} />
			</view>
		);
	}

	answers.set("isBold", "false");
	const { container, getByText } = render(<Toolbar />);
	expect(getByText("plain")).toBeInTheDocument();

	const element = editorElement(container);
	emit(element, "ready", { editorId: element.getAttribute("editor-id") });
	answers.set("isBold", "true");
	emit(element, "change", {
		document: false,
		selection: true,
		composition: false,
	});

	expect(getByText("bold")).toBeInTheDocument();
});
