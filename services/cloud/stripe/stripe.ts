/**
 * A minimal Stripe REST client: form-encoded requests, JSON responses, and
 * a dry-run mode that reports writes instead of sending them.
 */

import { API_VERSION } from "./catalog.ts";

export type Json = Record<string, unknown>;
type Params = Record<string, unknown>;

/** Flatten nested parameters into Stripe's `a[b][0]=c` form encoding. */
export function encode(params: Params, prefix = ""): [string, string][] {
	return Object.entries(params).flatMap(([key, value]) => {
		const name = prefix ? `${prefix}[${key}]` : key;

		if (value === undefined || value === null) return [];
		if (Array.isArray(value)) {
			return value.flatMap((item, index) =>
				typeof item === "object"
					? encode(item as Params, `${name}[${index}]`)
					: [[`${name}[${index}]`, String(item)] as [string, string]]
			);
		}
		if (typeof value === "object") return encode(value as Params, name);

		return [[name, String(value)]];
	});
}

export class Stripe {
	#key: string;
	#dryRun: boolean;
	#base: string;

	constructor(key: string, options: { dryRun: boolean; base?: string }) {
		this.#key = key;
		this.#dryRun = options.dryRun;
		this.#base = options.base ?? "https://api.stripe.com";
	}

	/** Whether writes are only reported. */
	get dryRun(): boolean {
		return this.#dryRun;
	}

	/** `GET path` with query parameters. */
	async get(path: string, params: Params = {}): Promise<Json> {
		const query = new URLSearchParams(encode(params)).toString();
		return await this.#request("GET", `${path}${query ? `?${query}` : ""}`);
	}

	/** Every object of a list endpoint, following pagination. */
	async list(path: string, params: Params = {}): Promise<Json[]> {
		const items: Json[] = [];
		let startingAfter: string | undefined;

		do {
			const page = await this.get(path, {
				...params,
				limit: 100,
				starting_after: startingAfter,
			});
			const data = page.data as Json[];
			items.push(...data);
			startingAfter = page.has_more
				? String(data[data.length - 1].id)
				: undefined;
		} while (startingAfter);

		return items;
	}

	/** `POST path`, or a report of it in a dry run. */
	async post(path: string, params: Params, description: string): Promise<Json> {
		if (this.#dryRun) {
			console.log(`  would ${description}`);
			return { id: `(dry run: ${description})` };
		}

		console.log(`  ${description}`);
		return await this.#request(
			"POST",
			path,
			new URLSearchParams(encode(params)),
		);
	}

	async #request(
		method: string,
		path: string,
		body?: URLSearchParams,
	): Promise<Json> {
		const response = await fetch(`${this.#base}${path}`, {
			method,
			headers: {
				authorization: `Bearer ${this.#key}`,
				"stripe-version": API_VERSION,
				"content-type": "application/x-www-form-urlencoded",
			},
			body,
		});
		const json = (await response.json()) as Json;

		if (!response.ok) {
			const error = json.error as Json | undefined;
			throw new Error(
				`${method} ${path}: ${response.status} ${error?.message ?? ""}`,
			);
		}

		return json;
	}
}
