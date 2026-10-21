import { assertEquals, assertThrows } from "@std/assert";
import { fluid, pageColor, themedCss } from "../../src/lib/cv-doc-css.ts";

Deno.test("fluid replaces exactly the width the stylesheet was rendered at", () => {
	assertEquals(
		fluid(".cv-doc{width:640px;color:red}.cv-check{width:14px;}", 640),
		".cv-doc{width:auto;color:red}.cv-check{width:14px;}",
	);
	assertThrows(() => fluid(".cv-doc{width:320px;}", 640), Error, "width:640px");
});

Deno.test("pageColor fails loudly when the stylesheet has no page colour", () => {
	assertEquals(
		pageColor(".cv-doc{margin:0;background-color:#1A1817;}"),
		"#1A1817",
	);
	assertThrows(() => pageColor(".cv-m-code{background-color:#eeeeff;}"));
});

Deno.test("themedCss applies the dark stylesheet wherever the site is dark", () => {
	assertEquals(
		themedCss(".cv-doc{color:#000}", ".cv-doc{color:#fff}"),
		".cv-doc{color:#000}" +
			'@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){.cv-doc{color:#fff}}}' +
			':root[data-theme="dark"]{.cv-doc{color:#fff}}',
	);
});
