import { assertEquals } from "@std/assert";

import { API_VERSION, PRICES, PRODUCTS, WEBHOOK_EVENTS } from "./catalog.ts";
import { encode } from "./stripe.ts";

Deno.test("parameters flatten into Stripe's form encoding", () => {
	assertEquals(
		encode({
			recurring: { interval: "month", meter: "mtr_1" },
			tiers: [{ up_to: 10_000, unit_amount: 0 }, {
				up_to: "inf",
				unit_amount_decimal: "0.2",
			}],
			enabled_events: ["a", "b"],
			skipped: undefined,
		}),
		[
			["recurring[interval]", "month"],
			["recurring[meter]", "mtr_1"],
			["tiers[0][up_to]", "10000"],
			["tiers[0][unit_amount]", "0"],
			["tiers[1][up_to]", "inf"],
			["tiers[1][unit_amount_decimal]", "0.2"],
			["enabled_events[0]", "a"],
			["enabled_events[1]", "b"],
		],
	);
});

Deno.test("every plan price has a lookup key the server maps to a plan", () => {
	const keys = PRICES.map((price) => price.lookupKey);
	assertEquals(new Set(keys).size, keys.length, "lookup keys are unique");

	for (const key of keys) {
		assertEquals(/^canvist_(builder|scale)_/.test(key), true, key);
	}
});

Deno.test("checkout finds every price it sells", () => {
	const keys = new Set(PRICES.map((price) => price.lookupKey));

	for (const plan of ["builder", "scale"]) {
		for (const suffix of ["monthly", "mad_overage", "conversion_overage"]) {
			assertEquals(
				keys.has(`canvist_${plan}_${suffix}`),
				true,
				`${plan} ${suffix}`,
			);
		}
	}
});

Deno.test("every product carries a tax code", () => {
	for (const product of PRODUCTS) {
		assertEquals(/^txcd_\d{8}$/.test(product.taxCode), true, product.key);
	}
});

const server = (path: string) =>
	Deno.readTextFileSync(new URL(`../src/metering/${path}`, import.meta.url));

Deno.test("the server pins the same API version", () => {
	assertEquals(
		server("stripe.rs").includes(`API_VERSION: &str = "${API_VERSION}"`),
		true,
	);
});

Deno.test("the webhook subscribes to exactly the events the server handles", () => {
	const [source] = server("stripe_webhook.rs").split("#[cfg(test)]");
	const handled = [...source.matchAll(/"([a-z_]+\.[a-z_.]+)"/g)].map((match) =>
		match[1]
	);

	assertEquals(new Set(handled), new Set(WEBHOOK_EVENTS));
});
