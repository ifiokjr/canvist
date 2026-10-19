/**
 * Where things go in each cut. Landscape puts the caption beside the page;
 * portrait and square put it above.
 */

import type { Profile } from "./data";

export type Format = "landscape" | "portrait" | "square";

export interface Region {
	readonly x: number;
	readonly y: number;
	readonly width: number;
	readonly height: number;
}

export interface Stage {
	readonly width: number;
	readonly height: number;
	/** The footage profile whose page width fits the sheet. */
	readonly profile: Profile;
	/** The caption column. */
	readonly caption: Region;
	/** The Paper sheet the document sits on. */
	readonly sheet: Region;
	/** Padding between the sheet's edge and the page. */
	readonly inset: number;
	/** Headline size. */
	readonly headline: number;
}

export const STAGES: Record<Format, Stage> = {
	landscape: {
		width: 1920,
		height: 1080,
		profile: "wide",
		caption: { x: 96, y: 150, width: 470, height: 780 },
		sheet: { x: 616, y: 96, width: 1216, height: 888 },
		inset: 48,
		headline: 66,
	},
	portrait: {
		width: 1080,
		height: 1920,
		profile: "narrow",
		caption: { x: 80, y: 230, width: 920, height: 580 },
		sheet: { x: 40, y: 840, width: 1000, height: 1000 },
		inset: 40,
		headline: 92,
	},
	square: {
		width: 1080,
		height: 1080,
		profile: "narrow",
		caption: { x: 80, y: 52, width: 920, height: 150 },
		sheet: { x: 40, y: 240, width: 1000, height: 800 },
		inset: 40,
		headline: 50,
	},
};
