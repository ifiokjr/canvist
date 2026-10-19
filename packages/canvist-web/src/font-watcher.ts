/**
 * Keeping measurements in step with the page's web fonts.
 *
 * The canvas resolves font families through `document.fonts`, so every face
 * the page declares with `@font-face` or adds as a `FontFace` is usable with
 * no configuration. But a face that is still loading measures and paints as
 * its fallback. The watcher asks the browser to load the faces layout
 * actually used, for the characters it used them for (so `unicode-range`
 * subsets load only when needed), and reports each family as its faces
 * finish loading, so layout forgets just those measurements.
 *
 * @module
 */
/** The family name of a face, without the quotes `FontFace.family` may keep. */
export function familyName(face: FontFace): string {
	return face.family.replace(/^["']|["']$/g, "");
}

/** Watches one `FontFaceSet` on behalf of one editor. */
export class FontWatcher {
	readonly #fonts: FontFaceSet;
	readonly #loaded: (families: readonly string[]) => void;
	/** Characters already checked or loaded, per CSS font. */
	readonly #seen = new Map<string, Set<string>>();
	readonly #pending = new Set<Promise<void>>();
	readonly #onLoadingDone: (event: FontFaceSetLoadEvent) => void;

	constructor(
		fonts: FontFaceSet,
		loaded: (families: readonly string[]) => void,
	) {
		this.#fonts = fonts;
		this.#loaded = loaded;
		this.#onLoadingDone = (event) => {
			const families = event.fontfaces.map(familyName);

			if (families.length > 0) {
				loaded(families);
			}
		};
		fonts.addEventListener("loadingdone", this.#onLoadingDone);
	}

	/** Whether faces requested by the watcher are still loading. */
	get loading(): boolean {
		return this.#pending.size > 0;
	}

	/** Resolve once every face the watcher asked for has loaded or failed. */
	async settled(): Promise<void> {
		while (this.#pending.size > 0) {
			await Promise.allSettled([...this.#pending]);
		}
	}

	/** Load whatever faces the fonts in `used` need for their text and do not have yet. */
	request(used: ReadonlyMap<string, string>): void {
		for (const [font, text] of used) {
			const fresh = this.#unseen(font, text);

			if (fresh.length === 0 || this.#fonts.check(font, fresh)) {
				continue;
			}

			this.#load(font, fresh);
		}
	}

	/**
	 * The characters of `text` not yet checked for `font`, marking them seen.
	 * Empty text still checks one character, so the font's primary face loads.
	 */
	#unseen(font: string, text: string): string {
		const seen = this.#seen.get(font) ?? new Set<string>();
		this.#seen.set(font, seen);
		let fresh = "";

		for (const character of text.length === 0 ? "x" : text) {
			if (!seen.has(character)) {
				seen.add(character);
				fresh += character;
			}
		}

		return fresh;
	}

	/** Load the faces of `font` that cover `characters`, reporting their families once loaded. */
	#load(font: string, characters: string): void {
		const pending: Promise<void> = this.#fonts.load(font, characters).then(
			(faces) => {
				if (faces.length > 0) {
					this.#loaded(faces.map(familyName));
				}
			},
			// A face that fails to load keeps its fallback, as CSS does.
			() => undefined,
		).finally(() => this.#pending.delete(pending));
		this.#pending.add(pending);
	}

	/** Stop listening. */
	dispose(): void {
		this.#fonts.removeEventListener("loadingdone", this.#onLoadingDone);
	}
}
