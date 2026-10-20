/**
 * Type-level tests, checked by `tsc --noEmit` and never run. Every
 * `@ts-expect-error` must flag a real error, or the check fails, so these
 * pin both what is allowed and what is not.
 *
 * @module
 */
import {
	createEngine as createFullEngine,
	highlight,
	starterKit,
} from "../src/full.ts";
import {
	bold,
	createEngine,
	type DocJson,
	heading,
	type Registered,
	type StarterKit,
} from "../src/mod.ts";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
	<T>() => T extends B ? 1 : 2 ? true : false;
type Kit = Registered<StarterKit>;

function assertType<Condition extends true>(_condition?: Condition): void {}

export async function onlyRegisteredExtensions(): Promise<void> {
	const engine = await createEngine([bold, heading]);
	const doc = engine.create();

	doc.commands.toggleBold();
	doc.commands.setParagraph();
	doc.can.toggleHeading({ level: 3 });
	assertType<Equal<ReturnType<typeof doc.query.isBold>, boolean>>();
	assertType<
		Equal<
			ReturnType<typeof doc.query.headingLevel>,
			1 | 2 | 3 | 4 | 5 | 6 | null
		>
	>();

	// @ts-expect-error italic is not registered
	doc.commands.toggleItalic();
	// @ts-expect-error italic is not registered
	doc.query.isItalic();
	// @ts-expect-error italic is not registered
	doc.run("toggleItalic");
	// @ts-expect-error lists are not registered
	doc.can.toggleBulletList();

	engine.fromJson({
		type: "doc",
		content: [{
			type: "heading",
			attrs: { level: 2 },
			content: [{ type: "text", text: "Hi", marks: [{ type: "bold" }] }],
		}],
	});
	// @ts-expect-error blockquote is not a registered block
	engine.fromJson({ type: "doc", content: [{ type: "blockquote" }] });
	const italic = {
		type: "text",
		text: "x",
		marks: [{ type: "italic" }],
	} as const;
	const paragraph = { type: "paragraph", content: [italic] } as const;
	// @ts-expect-error italic is not a registered mark
	engine.fromJson({ type: "doc", content: [paragraph] });
}

export async function argumentsFollowTheMetadata(): Promise<void> {
	const doc = (await createEngine()).create();

	doc.commands.setHeading({ level: 6 });
	doc.run("setHeading", { level: 1 });
	doc.commands.setLink({ href: "https://canvist.dev" });
	doc.commands.setCodeBlock();
	doc.commands.setCodeBlock({ language: null });
	doc.commands.setTaskChecked({ checked: true });

	// @ts-expect-error heading levels run from 1 to 6
	doc.commands.setHeading({ level: 7 });
	// @ts-expect-error the level is required
	doc.commands.setHeading();
	// @ts-expect-error the level is a number
	doc.run("setHeading", { level: "2" });
	// @ts-expect-error links need an href
	doc.commands.setLink({});
	// @ts-expect-error toggleBold takes no arguments
	doc.commands.toggleBold({ force: true });
	// @ts-expect-error checked is a boolean
	doc.commands.setTaskChecked({ checked: 1 });
}

export function documentsUseRegisteredNodes(): void {
	const json: DocJson<Registered<StarterKit>> = {
		type: "doc",
		content: [
			{
				type: "taskList",
				content: [{ type: "taskItem", attrs: { checked: true }, content: [] }],
			},
			{
				type: "paragraph",
				content: [{ type: "hardBreak" }, {
					type: "text",
					text: "x",
					marks: [{ type: "link", attrs: { href: "/" } }],
				}],
			},
		],
	};

	const unchecked = { type: "taskItem", attrs: { checked: "yes" } } as const;
	// @ts-expect-error taskItem.checked is a boolean
	const wrongAttr: DocJson<Kit> = { type: "doc", content: [unchecked] };
	// @ts-expect-error the root is a doc
	const wrongRoot: DocJson<Registered<StarterKit>> = { type: "paragraph" };

	void [json, wrongAttr, wrongRoot];
}

export async function extensionsMustExist(): Promise<void> {
	// @ts-expect-error extensions are descriptors, not names
	await createEngine(["bold"]);
}

export async function extrasNeedTheFullModule(): Promise<void> {
	// @ts-expect-error highlight is only in @canvist/headless/full
	await createEngine([bold, highlight]);

	const engine = await createFullEngine([...starterKit, highlight]);
	engine.create().commands.toggleHighlight();
}
