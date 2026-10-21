/**
 * Type-level tests for the formatting extensions beyond the StarterKit,
 * checked by `tsc --noEmit` and never run. Every `@ts-expect-error` must
 * flag a real error, or the check fails.
 *
 * @module
 */
import {
	color,
	createEngine,
	type DocJson,
	fontSize,
	highlight,
	type Registered,
	subscript,
	textStyle,
} from "../src/mod.ts";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
	<T>() => T extends B ? 1 : 2 ? true : false;

function assertType<Condition extends true>(_condition?: Condition): void {}

export async function formattingCommandsAreTyped(): Promise<void> {
	const engine = await createEngine([subscript, highlight, textStyle, color]);
	const doc = engine.create();

	doc.commands.toggleSubscript();
	doc.commands.setHighlight();
	doc.commands.setHighlight({ color: null });
	doc.commands.toggleHighlight({ color: "#fef08a" });
	doc.commands.setColor({ color: "#958df1" });
	doc.can.unsetColor();
	doc.run("removeEmptyTextStyle");

	assertType<Equal<ReturnType<typeof doc.query.isSubscript>, boolean>>();
	assertType<
		Equal<ReturnType<typeof doc.query.highlightColor>, string | null>
	>();
	assertType<Equal<ReturnType<typeof doc.query.color>, string | null>>();

	// @ts-expect-error a colour is required
	doc.commands.setColor();
	// @ts-expect-error a colour is a string
	doc.commands.setColor({ color: 7 });
	// @ts-expect-error a highlight colour is a string or null
	doc.commands.setHighlight({ color: true });
	// @ts-expect-error unsetColor takes no arguments
	doc.commands.unsetColor({ color: "red" });
	// @ts-expect-error superscript is not registered
	doc.commands.toggleSuperscript();
	// @ts-expect-error fontSize is not registered
	doc.query.fontSize();
}

export function formattingMarksAreTypedInJson(): void {
	type Kit = Registered<[typeof highlight, typeof textStyle, typeof fontSize]>;
	const json: DocJson<Kit> = {
		type: "doc",
		content: [{
			type: "paragraph",
			content: [{
				type: "text",
				text: "x",
				marks: [
					{ type: "highlight", attrs: { color: null } },
					{ type: "textStyle", attrs: { fontSize: "18px" } },
				],
			}],
		}],
	};
	void json;

	const wrongAttr: DocJson<Kit> = {
		type: "doc",
		content: [{
			type: "paragraph",
			content: [{
				type: "text",
				text: "x",
				// @ts-expect-error fontSize is a string
				marks: [{ type: "textStyle", attrs: { fontSize: 18 } }],
			}],
		}],
	};
	void wrongAttr;

	const unregistered: DocJson<Kit> = {
		type: "doc",
		content: [{
			type: "paragraph",
			content: [{
				type: "text",
				text: "x",
				// @ts-expect-error subscript is not registered
				marks: [{ type: "subscript" }],
			}],
		}],
	};
	void unregistered;
}
