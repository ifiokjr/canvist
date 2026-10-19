/**
 * The brand masters from `assets/brand/vector`, inlined so the logo paints in
 * the first frame with no extra request.
 *
 * The light and `-dark` files differ only in their two fills, so the site
 * inlines the light master and maps those fills to theme tokens. That keeps
 * one copy of the geometry and lets `light-dark()` pick the right pair.
 */
import lockupSvg from "../../../assets/brand/vector/lockup.svg?raw";
import markSvg from "../../../assets/brand/vector/mark.svg?raw";

const INK = /fill="#17141F"/gi;
const PAINT = /fill="#3B2CF5"/gi;

/** Strips the master's fixed size, title and fills so CSS controls them. */
function themable(svg: string, label: string): string {
	return svg
		.replace(/\s(width|height)="[^"]*"/g, "")
		.replace(/<title>[^<]*<\/title>\s*/, "")
		.replace(/aria-label="[^"]*"/, `aria-label="${label}"`)
		.replace(INK, 'fill="var(--logo-ink, currentColor)"')
		.replace(
			PAINT,
			'fill="var(--logo-paint, currentColor)" class="logo-caret"',
		);
}

export const lockup = themable(lockupSvg, "canvist");
export const mark = themable(markSvg, "canvist");

export const GITHUB_URL = "https://github.com/ifiokjr/canvist";
