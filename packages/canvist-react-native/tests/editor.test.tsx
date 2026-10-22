/**
 * The JavaScript half of `@canvist/react-native`: the props the native view
 * gets, the typed editor over the `CanvistEditors` module, and the events
 * that drive subscribers. The native module and the view's commands are
 * fakes that record what reaches them.
 */
import { describe, expect, jest, test } from "@jest/globals";
import { act, fireEvent, render, screen } from "@testing-library/react-native";
import { Text } from "react-native";

import { answers, calls, fakeCommands, fakeModule } from "./fakes.ts";

jest.mock("../src/NativeCanvistEditors.ts", () => ({
	__esModule: true,
	default: jest.requireActual<typeof import("./fakes.ts")>("./fakes.ts")
		.fakeModule,
}));

jest.mock("../src/CanvistEditorViewNativeComponent.ts", () => ({
	__esModule: true,
	...jest.requireActual<
		typeof import("../src/CanvistEditorViewNativeComponent.ts")
	>("../src/CanvistEditorViewNativeComponent.ts"),
	Commands: jest.requireActual<typeof import("./fakes.ts")>("./fakes.ts")
		.fakeCommands,
}));

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

type Kit = [typeof bold, typeof heading];

/** The native view's props, as React Native would send them. */
function nativeProps(testID = "editor") {
	return screen.getByTestId(testID).props;
}

/** Fire one of the native view's events. */
async function emit(name: string, payload: object, testID = "editor") {
	await fireEvent(screen.getByTestId(testID), name, { nativeEvent: payload });
}

/** Render an editor of bold and heading, and wait until it is ready. */
async function readyEditor(
	props: Partial<Parameters<typeof CanvistEditor<Kit>>[0]> = {},
): Promise<Editor<Kit>> {
	let ready: Editor<Kit> | null = null;
	await render(
		<CanvistEditor
			testID="editor"
			extensions={[bold, heading]}
			onReady={(editor) => {
				ready = editor;
			}}
			{...props}
		/>,
	);
	await emit("onEditorReady", { editorId: nativeProps().editorId });

	if (ready === null) {
		throw new Error("the editor never became ready");
	}

	return ready;
}

describe("CanvistEditor", () => {
	test("sends the native view its extensions and starting document", async () => {
		await render(
			<CanvistEditor
				testID="editor"
				extensions={[bold, heading]}
				content={{ format: "markdown", input: "# Notes" }}
				fontSize={19}
				accessibilityLabel="Note"
			/>,
		);
		const props = nativeProps();

		expect(props.editorId).toMatch(/^canvist-\d+$/);
		expect(props.extensions).toEqual(["bold", "heading"]);
		expect(props.contentFormat).toBe("markdown");
		expect(props.content).toBe("# Notes");
		expect(props.fontSize).toBe(19);
		expect(props.accessibilityLabel).toBe("Note");
	});

	test("sends ProseMirror JSON content as JSON", async () => {
		const doc = {
			type: "doc",
			content: [{ type: "paragraph", content: [{ type: "text", text: "Hi" }] }],
		} as const;
		await render(<CanvistEditor testID="editor" content={doc} />);

		expect(nativeProps().contentFormat).toBe("json");
		expect(JSON.parse(nativeProps().content)).toEqual(doc);
	});

	test("registers every StarterKit extension by default", async () => {
		await render(<CanvistEditor testID="editor" />);

		expect(nativeProps().extensions).toContain("taskList");
		expect(nativeProps().extensions).toContain("history");
	});

	test("gives each view its own editor id", async () => {
		await render(
			<>
				<CanvistEditor testID="first" />
				<CanvistEditor testID="second" />
			</>,
		);

		expect(nativeProps("first").editorId).not.toBe(
			nativeProps("second").editorId,
		);
	});

	test("typed commands and queries reach the editor its view registered", async () => {
		calls.length = 0;
		answers.set("headingLevel", "2");
		const editor = await readyEditor();
		const id = nativeProps().editorId;

		expect(editor.commands.toggleHeading({ level: 2 })).toBe(true);
		expect(editor.query.headingLevel()).toBe(2);
		expect(editor.can.toggleBold()).toBe(false);
		expect(editor.export("markdown")).toBe("Hello");
		expect(editor.selection).toEqual({
			anchor: { path: [0], offset: 1 },
			head: { path: [0], offset: 3 },
		});
		editor.select({ path: [0], offset: 0 });
		editor.setContent({ format: "markdown", input: "**Hi**" });
		expect(editor.undo()).toBe(true);

		expect(calls).toEqual([
			`run ${id} toggleHeading {"level":2}`,
			`query ${id} headingLevel null`,
			`can ${id} toggleBold null`,
			`export ${id} markdown`,
			`selection ${id}`,
			`select ${id} {"anchor":{"path":[0],"offset":0},"head":{"path":[0],"offset":0}}`,
			`load ${id} markdown **Hi**`,
			`undo ${id}`,
		]);
	});

	test("focus and blur go to the view's commands", async () => {
		const editor = await readyEditor();
		fakeCommands.focus.mockClear();
		fakeCommands.blur.mockClear();

		editor.focus();
		editor.blur();

		expect(fakeCommands.focus).toHaveBeenCalledTimes(1);
		expect(fakeCommands.blur).toHaveBeenCalledTimes(1);
	});

	test("change and focus events reach onUpdate and the editor's state", async () => {
		const onUpdate = jest.fn();
		const editor = await readyEditor({ onUpdate });

		await emit("onEditorChange", {
			document: true,
			selection: false,
			composition: false,
		});
		expect(onUpdate).toHaveBeenCalledTimes(1);

		await emit("onEditorFocus", { focused: true });
		expect(editor.focused).toBe(true);
		expect(onUpdate).toHaveBeenCalledTimes(2);
	});

	test("native failures arrive as canvist errors with their codes", async () => {
		const editor = await readyEditor();
		const run = jest.spyOn(fakeModule, "run").mockImplementationOnce(() => {
			throw new Error(
				"Exception in HostFunction: [canvist:invalidArgs] level must be 1 to 6",
			);
		});

		expect(() => editor.commands.toggleBold()).toThrow(
			expect.objectContaining({
				name: "CanvistError",
				code: "invalidArgs",
				message: "level must be 1 to 6",
			}),
		);
		run.mockRestore();
	});

	test("an editor the view could not create reports through onError", async () => {
		const onError = jest.fn();
		await render(<CanvistEditor testID="editor" onError={onError} />);

		await emit("onEditorError", {
			code: "config",
			message: "unknown extension",
		});

		const [error] = onError.mock.calls[0] ?? [];
		expect(isCanvistError(error)).toBe(true);
		expect(error).toMatchObject({
			code: "config",
			message: "unknown extension",
		});
	});
});

describe("useCanvistEditor and useEditorState", () => {
	function Toolbar() {
		const { editor, props } = useCanvistEditor({ extensions: [bold] });
		const active = useEditorState(editor, (e) => e.query.isBold(), false);

		return (
			<>
				<Text testID="state">{active ? "bold" : "plain"}</Text>
				<CanvistView {...props} testID="editor" />
			</>
		);
	}

	test("re-renders with the editor's state after each change", async () => {
		answers.set("isBold", "false");
		await render(<Toolbar />);
		expect(screen.getByTestId("state").props.children).toBe("plain");

		await emit("onEditorReady", { editorId: nativeProps().editorId });
		answers.set("isBold", "true");
		await emit("onEditorChange", {
			document: false,
			selection: true,
			composition: false,
		});

		expect(screen.getByTestId("state").props.children).toBe("bold");
	});

	test("the editor is null until the view is ready", async () => {
		let seen: unknown = "unset";
		function Probe() {
			const { editor, props } = useCanvistEditor();
			seen = editor;
			return <CanvistView {...props} testID="editor" />;
		}

		await render(<Probe />);
		expect(seen).toBeNull();

		await act(async () => {
			await emit("onEditorReady", { editorId: nativeProps().editorId });
		});
		expect(seen).not.toBeNull();
	});
});
