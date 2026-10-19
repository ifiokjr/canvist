/**
 * Build-time touches to the rendered markdown, so the article needs no
 * script for them: heading anchors, scrollable table regions, and a frame
 * around each code block for the copy button to sit in.
 *
 * The input is the HTML the markdown pipeline emits, whose shapes are fixed:
 * headings carry one `id`, and tables and `<pre>` blocks never nest.
 */

const HEADING = /<h([2-4]) id="([^"]+)">([\s\S]*?)<\/h\1>/g;

/**
 * The anchor has no text node (CSS draws the `#`), so search indexes and
 * section titles read the heading alone.
 */
function anchorHeading(
	_match: string,
	level: string,
	id: string,
	inner: string,
): string {
	const anchor =
		`<a class="docs-anchor" href="#${id}" aria-label="Link to this section"></a>`;
	return `<h${level} id="${id}">${inner}${anchor}</h${level}>`;
}

/** Wide tables scroll inside their own focusable region, never the page. */
function wrapTables(html: string): string {
	return html.replace(
		/<table>[\s\S]*?<\/table>/g,
		(table) =>
			`<div class="docs-table" role="region" aria-label="Table" tabindex="0">${table}</div>`,
	);
}

function frameCode(html: string): string {
	return html.replace(
		/<pre class="astro-code[\s\S]*?<\/pre>/g,
		(pre) => `<div class="docs-code">${pre}</div>`,
	);
}

export function enhanceDocsHtml(html: string): string {
	return frameCode(wrapTables(html.replace(HEADING, anchorHeading)));
}
