import { type Billing, estimate, plans } from "../data/plans.ts";

/** The slider is logarithmic: position 0 is zero, 1000 is one million MADs. */
const SLIDER_MAX = 1000;
const MADS_AT_MAX = 1_000_000;

export function sliderToMads(position: number): number {
	if (position <= 0) return 0;
	const raw = 10 ** ((position / SLIDER_MAX) * Math.log10(MADS_AT_MAX));
	// Two significant figures reads like a real estimate, not a random number.
	const magnitude = 10 ** Math.max(0, Math.floor(Math.log10(raw)) - 1);
	return Math.round(raw / magnitude) * magnitude;
}

export function madsToSlider(mads: number): number {
	if (mads <= 1) return 0;
	return Math.min(
		SLIDER_MAX,
		Math.round((Math.log10(mads) / Math.log10(MADS_AT_MAX)) * SLIDER_MAX),
	);
}

const usd = new Intl.NumberFormat("en-US", {
	style: "currency",
	currency: "USD",
	maximumFractionDigits: 2,
	minimumFractionDigits: 0,
});
const count = new Intl.NumberFormat("en-US");

export function mountPricing(scope: ParentNode): void {
	const range = scope.querySelector<HTMLInputElement>("[data-mads-range]");
	const number = scope.querySelector<HTMLInputElement>("[data-mads-number]");
	const osi = scope.querySelector<HTMLInputElement>("[data-opt-osi]");
	const small = scope.querySelector<HTMLInputElement>("[data-opt-small]");
	const planOut = scope.querySelector<HTMLElement>("[data-calc-plan]");
	const totalOut = scope.querySelector<HTMLElement>("[data-calc-total]");
	const detailOut = scope.querySelector<HTMLElement>("[data-calc-detail]");
	const billingButtons = [
		...scope.querySelectorAll<HTMLButtonElement>("[data-billing-option]"),
	];
	if (
		!range || !number || !osi || !small || !planOut || !totalOut || !detailOut
	) {
		throw new Error("Pricing calculator markup is incomplete");
	}

	let billing: Billing = "monthly";
	let mads = Number(number.value);

	function render(): void {
		if (!planOut || !totalOut || !detailOut || !osi || !small) return;
		const discounts = { openSource: osi.checked, smallTeam: small.checked };
		const result = estimate(mads, billing, discounts);

		if (result.contract || !result.best) {
			planOut.textContent = "Enterprise";
			totalOut.textContent = "Let's talk";
			detailOut.textContent = `At ${
				count.format(mads)
			} MADs a contract with custom limits costs less than self-serve overage.`;
		} else {
			const { best } = result;
			planOut.textContent = best.plan.name;
			totalOut.textContent = usd.format(best.total);
			const parts = [
				`per month, ${
					billing === "annual" ? "billed annually" : "billed monthly"
				}`,
			];
			if (best.overage > 0) {
				parts.push(`includes ${usd.format(best.overage)} of overage`);
			}
			if (discounts.smallTeam) parts.push("50% off for the first year");
			if (discounts.openSource && best.plan.id === "builder") {
				parts.push("Builder is free for open source");
			}
			detailOut.textContent = parts.join(" · ");
		}

		const totals = plans.map((plan) => result.quotes[plan.id]?.total);
		const max = Math.max(
			1,
			...totals.filter((value): value is number => value !== undefined),
		);
		for (const plan of plans) {
			const row = scope.querySelector<HTMLElement>(
				`[data-calc-bar="${plan.id}"]`,
			);
			const fill = row?.querySelector<HTMLElement>(".calc__bar-fill");
			const value = row?.querySelector<HTMLElement>(".calc__bar-value");
			if (!row || !fill || !value) continue;
			const quote = result.quotes[plan.id];
			fill.style.setProperty(
				"--fill",
				quote ? String(Math.max(0.015, quote.total / max)) : "0",
			);
			value.textContent = quote
				? usd.format(quote.total)
				: plan.id === "enterprise"
				? "custom"
				: `over ${count.format(plan.includedMads ?? 0)}`;
			row.toggleAttribute(
				"data-best",
				!result.contract && result.best?.plan.id === plan.id,
			);
		}

		for (const card of scope.querySelectorAll<HTMLElement>("[data-plan]")) {
			const id = card.dataset["plan"];
			const highlighted = result.contract
				? id === "enterprise"
				: id === result.best?.plan.id;
			card.toggleAttribute("data-highlighted", highlighted);
		}
	}

	function setBilling(next: Billing): void {
		billing = next;
		for (const button of billingButtons) {
			button.setAttribute(
				"aria-checked",
				String(button.dataset["billingOption"] === next),
			);
			button.tabIndex = button.dataset["billingOption"] === next ? 0 : -1;
		}
		for (
			const amount of scope.querySelectorAll<HTMLElement>(
				"[data-price-monthly]",
			)
		) {
			const value = Number(
				next === "annual"
					? amount.dataset["priceAnnual"]
					: amount.dataset["priceMonthly"],
			);
			amount.textContent = usd.format(value);
		}
		render();
	}

	range.value = String(madsToSlider(mads));
	range.addEventListener("input", () => {
		mads = sliderToMads(Number(range.value));
		number.value = String(mads);
		render();
	});
	number.addEventListener("input", () => {
		const value = Math.max(0, Math.floor(Number(number.value)));
		if (!Number.isFinite(value)) return;
		mads = value;
		range.value = String(madsToSlider(mads));
		render();
	});
	osi.addEventListener("change", render);
	small.addEventListener("change", render);
	for (const button of billingButtons) {
		button.addEventListener(
			"click",
			() =>
				setBilling(
					button.dataset["billingOption"] === "annual" ? "annual" : "monthly",
				),
		);
		button.addEventListener("keydown", (event) => {
			if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
			event.preventDefault();
			const next: Billing = billing === "annual" ? "monthly" : "annual";
			setBilling(next);
			billingButtons.find((other) => other.dataset["billingOption"] === next)
				?.focus();
		});
	}
	render();
}
