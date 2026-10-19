import { assertEquals, assertExists } from "@std/assert";
import { estimate, plans, quote } from "../../src/data/plans.ts";

const BUSINESS = new URL("../../../docs/src/v2/business.md", import.meta.url);
const none = { openSource: false, smallTeam: false };

function money(cell: string): number[] {
	return [...cell.matchAll(/\$([\d,.]+)/g)].map((match) =>
		Number(match[1]?.replace(/,/g, ""))
	);
}

Deno.test("plans match the provisional table in docs/src/v2/business.md", async () => {
	const markdown = await Deno.readTextFile(BUSINESS);
	const rows = markdown
		.split("\n")
		.filter((line) => /^\| (Hobby|Builder|Scale|Enterprise) /.test(line))
		.map((line) => line.split("|").slice(1, -1).map((cell) => cell.trim()));
	assertEquals(rows.length, 4, "business.md should list four plans");

	for (const [name = "", price = "", included = "", overage = ""] of rows) {
		const plan = plans.find((entry) => entry.name === name);
		assertExists(plan, `website has no ${name} plan`);
		const [monthly, annual] = money(price);
		assertEquals(plan.monthly, monthly, `${name} monthly price`);
		if (annual !== undefined) {
			assertEquals(plan.annual, annual, `${name} annual price`);
		}
		const mads = Number(included.replace(/,/g, ""));
		assertEquals(
			plan.includedMads,
			Number.isNaN(mads) ? undefined : mads,
			`${name} included MADs`,
		);
		assertEquals(plan.overagePer1000, money(overage)[0], `${name} overage`);
	}
});

Deno.test("hobby is free up to its cap and unavailable past it", () => {
	const hobby = plans.find((plan) => plan.id === "hobby");
	assertExists(hobby);
	assertEquals(quote(hobby, 1_000, "monthly", none)?.total, 0);
	assertEquals(quote(hobby, 1_001, "monthly", none), undefined);
});

Deno.test("overage is billed per started 1,000 MADs", () => {
	const builder = plans.find((plan) => plan.id === "builder");
	assertExists(builder);
	assertEquals(quote(builder, 10_001, "monthly", none)?.overage, 2);
	assertEquals(quote(builder, 25_000, "monthly", none)?.total, 29 + 15 * 2);
	assertEquals(quote(builder, 25_000, "annual", none)?.total, 24 + 15 * 2);
});

Deno.test("estimate picks the cheapest plan and hands large usage to a contract", () => {
	assertEquals(estimate(500, "monthly", none).best?.plan.id, "hobby");
	assertEquals(estimate(25_000, "monthly", none).best?.plan.id, "builder");
	// Builder at 200k: 29 + 190 * 2 = 409; Scale: 349 + 50 * 1.5 = 424. Builder still wins.
	assertEquals(estimate(200_000, "monthly", none).best?.plan.id, "builder");
	assertEquals(estimate(400_000, "monthly", none).best?.plan.id, "scale");
	assertEquals(estimate(5_000_000, "monthly", none).contract, true);
});

Deno.test("discounts: Builder is free for open source, small teams pay half", () => {
	assertEquals(
		estimate(5_000, "monthly", { openSource: true, smallTeam: false }).best
			?.total,
		0,
	);
	assertEquals(
		estimate(25_000, "monthly", { openSource: false, smallTeam: true }).best
			?.total,
		(29 + 30) / 2,
	);
});
