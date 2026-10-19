/**
 * canvist Cloud's billing catalog in Stripe: the meters, products, prices,
 * Customer Portal configuration, and webhook events `setup.ts` creates.
 * Prices follow the provisional plans in `docs/src/v2/business.md`; amounts
 * are in US cents. Hobby is free and has no Stripe objects.
 */

/** The API version the setup script and the server both pin. */
export const API_VERSION = "2026-08-26.dahlia";

/** A Billing Meter, matched by its event name. */
export type MeterSpec = {
	eventName: string;
	displayName: string;
};

/** A product, matched by `metadata.canvist_key`. */
export type ProductSpec = {
	key: string;
	name: string;
	description: string;
	/** Stripe Tax product tax code. */
	taxCode: string;
};

/** A price, matched by its lookup key. Every price is tax-exclusive. */
export type PriceSpec = {
	lookupKey: string;
	product: string;
	nickname: string;
	/** Licensed prices bill a fixed amount each interval. */
	licensed?: { unitAmount: number; interval: "month" | "year" };
	/**
	 * Metered prices bill a meter's monthly sum: free up to `included`, then
	 * `unitAmountDecimal` cents each.
	 */
	metered?: { meter: string; included: number; unitAmountDecimal: string };
};

/** What the server reports as `STRIPE_METER_EVENT`. */
export const ACTIVE_DOCUMENT_METER = "canvist_active_document";
/** What the server reports as `STRIPE_CONVERSION_METER_EVENT`. */
export const CONVERSION_METER = "canvist_conversion";

export const METERS: MeterSpec[] = [
	{
		eventName: ACTIVE_DOCUMENT_METER,
		displayName: "canvist monthly active documents",
	},
	{ eventName: CONVERSION_METER, displayName: "canvist conversions" },
];

/**
 * Software as a service, business use: the code Stripe's guidance names for
 * SaaS sold to businesses. Pending the owner's confirmation against
 * https://docs.stripe.com/tax/tax-codes before taxes are switched on.
 */
const SAAS_BUSINESS_USE = "txcd_10103001";

export const PRODUCTS: ProductSpec[] = [
	{
		key: "sync",
		name: "canvist Sync",
		description: "Real-time collaboration, billed per monthly active document.",
		taxCode: SAAS_BUSINESS_USE,
	},
	{
		key: "convert",
		name: "canvist Convert",
		description: "Document conversion between JSON, HTML, Markdown, and text.",
		taxCode: SAAS_BUSINESS_USE,
	},
	{
		key: "history",
		name: "canvist History",
		description: "Snapshots, named versions, diff, and restore.",
		taxCode: SAAS_BUSINESS_USE,
	},
];

const MONTHLY_DOCUMENTS = { builder: 10_000, scale: 150_000 };

export const PRICES: PriceSpec[] = [
	{
		lookupKey: "canvist_builder_monthly",
		product: "sync",
		nickname: "Builder, monthly",
		licensed: { unitAmount: 2_900, interval: "month" },
	},
	{
		lookupKey: "canvist_builder_annual",
		product: "sync",
		nickname: "Builder, annual",
		licensed: { unitAmount: 24 * 12 * 100, interval: "year" },
	},
	{
		lookupKey: "canvist_scale_monthly",
		product: "sync",
		nickname: "Scale, monthly",
		licensed: { unitAmount: 34_900, interval: "month" },
	},
	{
		lookupKey: "canvist_scale_annual",
		product: "sync",
		nickname: "Scale, annual",
		licensed: { unitAmount: 299 * 12 * 100, interval: "year" },
	},
	{
		lookupKey: "canvist_builder_mad_overage",
		product: "sync",
		nickname: "Builder, $2 per 1,000 MADs over 10,000",
		metered: {
			meter: ACTIVE_DOCUMENT_METER,
			included: MONTHLY_DOCUMENTS.builder,
			unitAmountDecimal: "0.2",
		},
	},
	{
		lookupKey: "canvist_scale_mad_overage",
		product: "sync",
		nickname: "Scale, $1.50 per 1,000 MADs over 150,000",
		metered: {
			meter: ACTIVE_DOCUMENT_METER,
			included: MONTHLY_DOCUMENTS.scale,
			unitAmountDecimal: "0.15",
		},
	},
	{
		lookupKey: "canvist_builder_conversion_overage",
		product: "convert",
		nickname: "Builder, $0.01 per conversion",
		metered: { meter: CONVERSION_METER, included: 0, unitAmountDecimal: "1" },
	},
	{
		lookupKey: "canvist_scale_conversion_overage",
		product: "convert",
		nickname: "Scale, $0.005 per conversion",
		metered: { meter: CONVERSION_METER, included: 0, unitAmountDecimal: "0.5" },
	},
];

/**
 * The Customer Portal configuration, matched by `metadata.canvist_key`.
 * Plan switches stay off: the portal cannot update subscriptions with
 * metered prices, so `POST /v1/billing/plan` does them instead.
 */
export const PORTAL = {
	key: "portal",
	params: {
		business_profile: { headline: "canvist Cloud billing" },
		features: {
			customer_update: {
				enabled: true,
				allowed_updates: ["email", "address", "name", "tax_id"],
			},
			invoice_history: { enabled: true },
			payment_method_update: { enabled: true },
			subscription_cancel: {
				enabled: true,
				mode: "at_period_end",
				cancellation_reason: {
					enabled: true,
					options: [
						"too_expensive",
						"missing_features",
						"switched_service",
						"unused",
						"other",
					],
				},
			},
			subscription_update: { enabled: false },
		},
	},
};

/** The events `POST /v1/stripe/webhook` acts on. */
export const WEBHOOK_EVENTS = [
	"checkout.session.completed",
	"customer.subscription.created",
	"customer.subscription.updated",
	"customer.subscription.deleted",
	"customer.subscription.paused",
	"customer.subscription.resumed",
	"invoice.paid",
	"invoice.payment_failed",
];
