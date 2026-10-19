/**
 * The docs search modal. Pagefind's index only exists after `deno task build`
 * (it runs over `dist`), so the module is imported on first open and the dev
 * server explains why there are no results instead of failing silently.
 */

/** The slice of Pagefind's JS API (v1) this modal uses. */
interface PagefindSubResult {
	title: string;
	url: string;
	/** HTML: page text with the matched words wrapped in `<mark>`. */
	excerpt: string;
	locations: number[];
}

interface PagefindResultData {
	url: string;
	excerpt: string;
	meta: { title?: string };
	sub_results: PagefindSubResult[];
}

interface PagefindResult {
	id: string;
	data(): Promise<PagefindResultData>;
}

interface PagefindResponse {
	results: PagefindResult[];
}

interface Pagefind {
	options(options: { excerptLength?: number }): Promise<void>;
	init(): Promise<void>;
	/** Resolves `null` when a newer call superseded this one. */
	debouncedSearch(
		term: string,
		options?: Record<string, never>,
		debounceMs?: number,
	): Promise<PagefindResponse | null>;
}

interface SearchHit {
	title: string;
	section: string | undefined;
	url: string;
	excerpt: string;
}

const PAGEFIND_PATH = "/pagefind/pagefind.js";
const MAX_HITS = 8;

function isPagefind(value: unknown): value is Pagefind {
	return typeof value === "object" && value !== null &&
		"debouncedSearch" in value && typeof value.debouncedSearch === "function" &&
		"options" in value && typeof value.options === "function" &&
		"init" in value && typeof value.init === "function";
}

let pagefind: Promise<Pagefind> | undefined;

async function importPagefind(): Promise<Pagefind> {
	// A variable path keeps Vite from bundling a file that only exists in `dist`.
	const module: unknown = await import(/* @vite-ignore */ PAGEFIND_PATH);
	if (!isPagefind(module)) {
		throw new Error(`${PAGEFIND_PATH} is not the Pagefind API`);
	}
	await module.options({ excerptLength: 22 });
	await module.init();
	return module;
}

function loadPagefind(): Promise<Pagefind> {
	if (pagefind !== undefined) return pagefind;
	const loading = importPagefind();
	pagefind = loading;
	// A failed load is retried on the next search, not cached for the page's life.
	loading.catch(() => {
		if (pagefind === loading) pagefind = undefined;
	});
	return loading;
}

/** Built pages are `.html` files; the site links to them without the extension. */
function siteUrl(url: string): string {
	return url.replace(/\.html(?=$|[#?])/, "");
}

function toHit(data: PagefindResultData): SearchHit {
	const title = data.meta.title ?? siteUrl(data.url);
	// The section with the most matches is the best place to land.
	const best = data.sub_results.reduce<PagefindSubResult | undefined>(
		(
			top,
			sub,
		) => (top === undefined || sub.locations.length > top.locations.length
			? sub
			: top),
		undefined,
	);
	const section = best !== undefined && best.title !== title
		? best.title
		: undefined;
	return {
		title,
		section,
		url: siteUrl(best?.url ?? data.url),
		excerpt: best?.excerpt ?? data.excerpt,
	};
}

function required<T extends Element>(
	root: ParentNode,
	selector: string,
	type: new () => T,
): T {
	const element = root.querySelector(selector);
	if (!(element instanceof type)) {
		throw new Error(`Docs search is missing ${selector}`);
	}
	return element;
}

function isEditable(target: EventTarget | null): boolean {
	return target instanceof HTMLElement &&
		(target.isContentEditable || target instanceof HTMLInputElement ||
			target instanceof HTMLTextAreaElement ||
			target instanceof HTMLSelectElement);
}

export function mountDocsSearch(): void {
	const dialog = document.querySelector("#docs-search");
	if (!(dialog instanceof HTMLDialogElement)) return;
	const input = required(dialog, "input", HTMLInputElement);
	const list = required(dialog, "[role=listbox]", HTMLElement);
	const status = required(dialog, "[data-docs-search-status]", HTMLElement);
	const triggers = document.querySelectorAll<HTMLButtonElement>(
		"[data-docs-search-open]",
	);
	const idleMessage = status.textContent?.trim() ?? "";

	let hits: HTMLAnchorElement[] = [];
	let active = -1;
	let latest = "";

	const isMac = /Mac|iPhone|iPad/.test(
		navigator.platform || navigator.userAgent,
	);
	if (isMac) {
		for (const kbd of document.querySelectorAll("[data-docs-kbd]")) {
			kbd.textContent = "⌘K";
		}
	}

	function setActive(index: number): void {
		hits[active]?.setAttribute("aria-selected", "false");
		active = hits.length === 0 ? -1 : (index + hits.length) % hits.length;
		const hit = hits[active];
		if (hit === undefined) {
			input.removeAttribute("aria-activedescendant");
			return;
		}
		hit.setAttribute("aria-selected", "true");
		input.setAttribute("aria-activedescendant", hit.id);
		hit.scrollIntoView({ block: "nearest" });
	}

	function show(message: string, results: readonly SearchHit[] = []): void {
		status.textContent = message;
		list.replaceChildren(...results.map(renderHit));
		hits = [...list.querySelectorAll<HTMLAnchorElement>("[role=option]")];
		input.setAttribute("aria-expanded", String(hits.length > 0));
		active = -1;
		setActive(0);
	}

	function renderHit(hit: SearchHit, index: number): HTMLAnchorElement {
		const link = document.createElement("a");
		link.className = "docs-search__hit";
		link.id = `docs-search-hit-${index}`;
		link.href = hit.url;
		link.setAttribute("role", "option");
		link.setAttribute("aria-selected", "false");
		link.tabIndex = -1;
		const heading = document.createElement("span");
		heading.className = "docs-search__hit-title";
		heading.textContent = hit.title;
		if (hit.section !== undefined) {
			const section = document.createElement("span");
			section.className = "docs-search__hit-section";
			section.textContent = hit.section;
			heading.append(section);
		}
		const excerpt = document.createElement("span");
		excerpt.className = "docs-search__hit-excerpt";
		// Pagefind escapes the page text and adds only `<mark>` around matches.
		excerpt.innerHTML = hit.excerpt;
		link.append(heading, excerpt);
		link.addEventListener("pointermove", () => {
			if (active !== index) setActive(index);
		});
		return link;
	}

	async function search(term: string): Promise<void> {
		latest = term;
		if (term === "") {
			show(idleMessage);
			return;
		}
		let api: Pagefind;
		try {
			api = await loadPagefind();
		} catch {
			show(
				import.meta.env.DEV
					? "Search needs the Pagefind index, which only the production build creates. Run `deno task build`, then `deno task preview`."
					: "Search could not load. Check your connection and try again.",
			);
			return;
		}
		const response = await api.debouncedSearch(term, {}, 120);
		if (response === null || term !== latest) return;
		const data = await Promise.all(
			response.results.slice(0, MAX_HITS).map((result) => result.data()),
		);
		if (term !== latest) return;
		if (data.length === 0) {
			show(
				`Nothing matches “${term}”. Try a shorter word or a different spelling.`,
			);
			return;
		}
		const count = response.results.length;
		show(
			`${count} ${count === 1 ? "page matches" : "pages match"} “${term}”.`,
			data.map(toHit),
		);
	}

	function open(): void {
		if (dialog instanceof HTMLDialogElement && !dialog.open) dialog.showModal();
		input.select();
		// Start fetching the index while the reader types; `search` reports failures.
		loadPagefind().catch(() => undefined);
		for (const trigger of triggers) {
			trigger.setAttribute("aria-expanded", "true");
		}
	}

	for (const trigger of triggers) trigger.addEventListener("click", open);
	required(dialog, "[data-docs-search-close]", HTMLButtonElement)
		.addEventListener("click", () => dialog.close());
	dialog.addEventListener("close", () => {
		for (const trigger of triggers) {
			trigger.setAttribute("aria-expanded", "false");
		}
	});
	// A click on the dialog itself (not its panel) is a click on the backdrop.
	dialog.addEventListener("click", (event) => {
		if (event.target === dialog) dialog.close();
	});

	input.addEventListener("input", () => {
		void search(input.value.trim());
	});

	input.addEventListener("keydown", (event) => {
		if (event.key === "ArrowDown" || event.key === "ArrowUp") {
			event.preventDefault();
			setActive(active + (event.key === "ArrowDown" ? 1 : -1));
		} else if (event.key === "Enter") {
			const hit = hits[active];
			if (hit === undefined) return;
			event.preventDefault();
			hit.click();
		}
	});

	document.addEventListener("keydown", (event) => {
		const shortcut = event.key.toLowerCase() === "k" &&
			(event.metaKey || event.ctrlKey);
		const slash = event.key === "/" && !isEditable(event.target);
		if (!shortcut && !slash) return;
		event.preventDefault();
		if (dialog.open && shortcut) dialog.close();
		else open();
	});
}
