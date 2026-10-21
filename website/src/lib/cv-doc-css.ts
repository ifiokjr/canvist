/**
 * Fitting the stylesheet canvist's server renderer (`@canvist/headless/ssr`)
 * writes to the site: a fluid width, both colour schemes, and the page
 * colour the canvas paints behind the document.
 */

/**
 * `css` with `.cv-doc` sized by the element it sits in instead of fixed at
 * the width it was rendered at. The editor wraps to that element's width,
 * and the HTML wraps the same lines at any width (`pixel-render.ts` checks that).
 */
export function fluid(css: string, width: number): string {
	const fixed = `width:${width}px;`;
	if (css.split(fixed).length !== 2) {
		throw new Error(`Expected one "${fixed}" in canvist's stylesheet`);
	}
	return css.replace(fixed, "width:auto;");
}

/** The `.cv-doc` background in canvist's stylesheet. */
export function pageColor(css: string): string {
	const color = /\.cv-doc\{[^}]*background-color:(#[0-9a-f]{6})/i.exec(css)
		?.[1];
	if (color === undefined) {
		throw new Error("canvist's stylesheet sets no .cv-doc background");
	}
	return color;
}

/**
 * The light stylesheet, overridden by the dark one wherever the site is
 * dark: when `data-theme` pins it, or when the system is dark and nothing
 * pins light (as `src/styles/tokens.css` decides). Nesting scopes the dark
 * rules under `:root`, which also makes them win over the light ones.
 */
export function themedCss(light: string, dark: string): string {
	return [
		light,
		`@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){${dark}}}`,
		`:root[data-theme="dark"]{${dark}}`,
	].join("");
}
