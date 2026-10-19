/**
 * The smallest app that uses the editor, built by `vite.config.ts` to
 * measure what `@canvist/web` costs a page: core plus the StarterKit,
 * tree-shaken and minified as an app's bundler ships it.
 */
import { createEditor } from "../../src/mod.ts";

const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	await createEditor(element, {
		content: { format: "markdown", input: "# Hello" },
	});
}
