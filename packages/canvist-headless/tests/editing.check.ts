/**
 * Type-level tests for the editing helpers beyond the StarterKit, checked
 * by `tsc --noEmit` and never run. Every `@ts-expect-error` must flag a
 * real error, or the check fails.
 *
 * @module
 */
import {
	characterCount,
	createEngine,
	type DocJson,
	placeholder,
	type Registered,
	textAlign,
	typography,
} from "../src/mod.ts";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
	<T>() => T extends B ? 1 : 2 ? true : false;

function assertType<Condition extends true>(_condition?: Condition): void {}

export async function editingCommandsAreTyped(): Promise<void> {
	const engine = await createEngine([
		textAlign,
		typography,
		characterCount,
		placeholder,
	]);
	const doc = engine.create();

	doc.commands.setTextAlign({ alignment: "justify" });
	doc.commands.toggleTextAlign({ alignment: "left" });
	doc.commands.unsetTextAlign();
	doc.commands.setCharacterLimit({ limit: null });
	doc.commands.setCharacterLimit({ limit: 280 });
	doc.commands.setPlaceholder({ placeholder: "Write…" });

	assertType<
		Equal<
			ReturnType<typeof doc.query.textAlign>,
			"left" | "center" | "right" | "justify" | null
		>
	>();
	assertType<Equal<ReturnType<typeof doc.query.characters>, number>>();
	assertType<
		Equal<ReturnType<typeof doc.query.characterLimit>, number | null>
	>();
	assertType<Equal<ReturnType<typeof doc.query.placeholder>, string | null>>();

	// @ts-expect-error alignments are left, center, right, or justify
	doc.commands.setTextAlign({ alignment: "middle" });
	// @ts-expect-error an alignment is required
	doc.commands.toggleTextAlign();
	// @ts-expect-error the limit is a number or null
	doc.commands.setCharacterLimit({ limit: "10" });
	// @ts-expect-error the limit is required, even when null
	doc.commands.setCharacterLimit({});
	// @ts-expect-error placeholder text is a string
	doc.commands.setPlaceholder({ placeholder: null });
}

export function alignmentIsTypedInJson(): void {
	type Kit = Registered<[typeof textAlign]>;
	const json: DocJson<Kit> = {
		type: "doc",
		content: [{ type: "paragraph", attrs: { textAlign: "center" } }],
	};
	void json;

	const wrong: DocJson<Kit> = {
		type: "doc",
		// @ts-expect-error sideways is not an alignment
		content: [{ type: "paragraph", attrs: { textAlign: "sideways" } }],
	};
	void wrong;
}
