/** The parts of `subset-font` (HarfBuzz's subsetter in wasm) the font script uses. */
declare module "subset-font" {
	interface SubsetOptions {
		readonly targetFormat?: "sfnt" | "truetype" | "woff" | "woff2";
		readonly preserveNameIds?: readonly number[];
		readonly variationAxes?: Readonly<
			Record<string, number | { min: number; max: number; default?: number }>
		>;
	}

	export default function subsetFont(
		font: Uint8Array,
		text: string,
		options?: SubsetOptions,
	): Promise<Buffer>;
}
