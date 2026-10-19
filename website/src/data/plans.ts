/**
 * The provisional plans from `docs/src/cloud/billing.md`. They are not final:
 * every surface that shows them labels them as a preview.
 *
 * `tests/plans.test.ts` checks these values against the business doc.
 */

export type PlanId = "hobby" | "builder" | "scale" | "enterprise";

export interface Plan {
	id: PlanId;
	name: string;
	/** USD per month, billed monthly. `undefined` for custom pricing. */
	monthly?: number;
	/** USD per month, billed annually. */
	annual?: number;
	/** Monthly active documents included. `undefined` means custom. */
	includedMads?: number;
	/** USD per 1,000 MADs over the included amount; `undefined` is a hard cap. */
	overagePer1000?: number;
	pitch: string;
	features: string[];
}

export const plans: Plan[] = [
	{
		id: "hobby",
		name: "Hobby",
		monthly: 0,
		annual: 0,
		includedMads: 1_000,
		pitch: "Side projects and prototypes.",
		features: [
			"1,000 monthly active documents",
			"Hard cap: hosted features pause, the editor never goes read-only",
		],
	},
	{
		id: "builder",
		name: "Builder",
		monthly: 29,
		annual: 24,
		includedMads: 10_000,
		overagePer1000: 2,
		pitch: "A product with real users.",
		features: [
			"10,000 monthly active documents",
			"$2 per 1,000 MADs over",
			"Warnings, never automatic upgrades",
			"Free for OSI-licensed projects",
		],
	},
	{
		id: "scale",
		name: "Scale",
		monthly: 349,
		annual: 299,
		includedMads: 150_000,
		overagePer1000: 1.5,
		pitch: "Collaboration at the centre of your app.",
		features: [
			"150,000 monthly active documents",
			"$1.50 per 1,000 MADs over",
			"Warnings, never automatic upgrades",
		],
	},
	{
		id: "enterprise",
		name: "Enterprise",
		monthly: 2_000,
		pitch: "Custom limits, contracts and support.",
		features: [
			"From $2,000 a month, billed annually",
			"Custom MADs and negotiated overage",
		],
	},
];

/** Discounts from the business doc. */
export const SMALL_TEAM_DISCOUNT = 0.5;
export const SMALL_TEAM_MAX_PEOPLE = 5;

export type Billing = "monthly" | "annual";

export interface Discounts {
	/** OSI-licensed open-source projects get Builder free. */
	openSource: boolean;
	/** Companies under five people get 50% off for the first year. */
	smallTeam: boolean;
}

export interface Quote {
	plan: Plan;
	/** USD per month for the plan itself, after the open-source waiver. */
	base: number;
	/** USD per month for MADs over the included amount. */
	overage: number;
	/** USD per month, after every discount. */
	total: number;
}

export interface Estimate {
	/** Each plan's cost at this usage; `undefined` when the plan can't serve it. */
	quotes: Record<PlanId, Quote | undefined>;
	/** The cheapest self-serve plan, unless usage is past where a contract is cheaper. */
	best?: Quote;
	/** True when the cheapest self-serve plan costs at least the Enterprise floor. */
	contract: boolean;
}

/**
 * The monthly cost of one plan at a usage level, or `undefined` when the plan
 * cannot serve it (Hobby is capped) or is priced by contract (Enterprise).
 */
export function quote(
	plan: Plan,
	mads: number,
	billing: Billing,
	discounts: Discounts,
): Quote | undefined {
	if (plan.includedMads === undefined) return undefined;
	const listed = billing === "annual" ? plan.annual : plan.monthly;
	if (listed === undefined) return undefined;
	const over = Math.max(0, mads - plan.includedMads);
	if (over > 0 && plan.overagePer1000 === undefined) return undefined;
	const base = discounts.openSource && plan.id === "builder" ? 0 : listed;
	const overage = Math.ceil(over / 1000) * (plan.overagePer1000 ?? 0);
	const subtotal = base + overage;
	const total = discounts.smallTeam
		? subtotal * (1 - SMALL_TEAM_DISCOUNT)
		: subtotal;
	return { plan, base, overage, total };
}

export function estimate(
	mads: number,
	billing: Billing,
	discounts: Discounts,
): Estimate {
	const quotes = Object.fromEntries(
		plans.map((plan) => [plan.id, quote(plan, mads, billing, discounts)]),
	) as Record<
		PlanId,
		Quote | undefined
	>;
	const selfServe = Object.values(quotes).filter((entry): entry is Quote =>
		entry !== undefined
	);
	selfServe.sort((a, b) => a.total - b.total);
	const best = selfServe[0];
	const floor = plans.find((plan) => plan.id === "enterprise")?.monthly ??
		Number.POSITIVE_INFINITY;
	const contract = best === undefined || best.total >= floor;
	return contract || best === undefined
		? { quotes, contract: true }
		: { quotes, best, contract: false };
}
