import { assertEquals } from "@std/assert";

import { PRICES, WEBHOOK_EVENTS } from "./catalog.ts";
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
		assertEquals(/^canvist_(hobby|builder|scale)_/.test(key), true, key);
	}
});

Deno.test("the webhook subscribes to the events the server handles", () => {
	assertEquals(WEBHOOK_EVENTS, [
		"customer.subscription.created",
		"customer.subscription.updated",
		"customer.subscription.deleted",
	]);
});
