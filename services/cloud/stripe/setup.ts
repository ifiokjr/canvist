/**
 * Create or update canvist Cloud's billing catalog in a Stripe **test-mode**
 * account. Every object is found by a stable key first (meter event name,
 * product `metadata.canvist_key`, price lookup key, webhook URL), so running
 * it again changes nothing.
 *
 *   deno task stripe:setup --dry-run
 *   deno task stripe:setup [--account acct_…] [--webhook-url https://…/v1/stripe/webhook]
 *
 * The key comes from `STRIPE_SECRET_KEY`, or else from the Stripe CLI's
 * configured test-mode key. It refuses live keys and live accounts, and any
 * account whose name is not "canvist" unless `--account` names it. A new
 * webhook endpoint's signing secret goes straight into the Railway variable
 * `STRIPE_WEBHOOK_SECRET` of the `sync` service (`--railway-environment`,
 * default `staging`); secrets are never printed. See
 * `docs/src/cloud/billing.md`.
 */

import { parseArgs } from "@std/cli/parse-args";

import { METERS, PRICES, PRODUCTS, WEBHOOK_EVENTS } from "./catalog.ts";
import type { MeterSpec, PriceSpec, ProductSpec } from "./catalog.ts";
import { type Json, Stripe } from "./stripe.ts";

const STAGING_WEBHOOK =
	"https://sync-staging-0f98.up.railway.app/v1/stripe/webhook";
const ACCOUNT_NAME = "canvist";

type Options = {
	dryRun: boolean;
	account?: string;
	webhookUrl: string;
	railwayEnvironment: string;
};

function options(): Options {
	const args = parseArgs(Deno.args, {
		boolean: ["dry-run"],
		string: ["account", "webhook-url", "railway-environment"],
		default: {
			"webhook-url": STAGING_WEBHOOK,
			"railway-environment": "staging",
		},
	});

	return {
		dryRun: args["dry-run"],
		account: args.account,
		webhookUrl: args["webhook-url"],
		railwayEnvironment: args["railway-environment"],
	};
}

function fail(message: string): never {
	console.error(`refusing: ${message}`);
	Deno.exit(1);
}

/** `STRIPE_SECRET_KEY`, or the Stripe CLI's test-mode key. */
async function secretKey(): Promise<string> {
	const fromEnv = Deno.env.get("STRIPE_SECRET_KEY");
	if (fromEnv) return fromEnv;

	const output = await new Deno.Command("stripe", {
		args: ["config", "--list"],
		stdout: "piped",
		stderr: "null",
	})
		.output()
		.catch(() => fail("set STRIPE_SECRET_KEY or log the Stripe CLI in"));
	const config = new TextDecoder().decode(output.stdout);
	const key = config.match(/test_mode_api_key\s*=\s*['"]([^'"]+)['"]/)?.[1];

	return key ?? fail("the Stripe CLI has no test-mode key; run `stripe login`");
}

/** The account, after checking it is test mode and the canvist account. */
async function checkAccount(
	stripe: Stripe,
	key: string,
	expected?: string,
): Promise<Json> {
	if (!/^(sk|rk)_test_/.test(key)) {
		fail("only test-mode keys (sk_test_ or rk_test_) are used");
	}

	const balance = await stripe.get("/v1/balance");
	if (balance.livemode !== false) fail("the key is not in test mode");

	const account = await stripe.get("/v1/account");
	const settings = account.settings as
		| { dashboard?: { display_name?: string } }
		| undefined;
	const profile = account.business_profile as { name?: string } | undefined;
	const names = [settings?.dashboard?.display_name, profile?.name].filter(
		Boolean,
	) as string[];
	const named = names.some((name) =>
		name.trim().toLowerCase() === ACCOUNT_NAME
	);
	const chosen = expected !== undefined && account.id === expected;

	if (!(named || chosen)) {
		fail(
			`account ${account.id} (${
				names.join(", ") || "unnamed"
			}) is not "${ACCOUNT_NAME}"; pass --account ${account.id} to use it anyway`,
		);
	}

	console.log(`account ${account.id} (${names.join(", ")}), test mode`);
	return account;
}

async function ensureMeter(
	stripe: Stripe,
	existing: Json[],
	spec: MeterSpec,
): Promise<string> {
	const found = existing.find((meter) =>
		meter.event_name === spec.eventName && meter.status === "active"
	);
	if (found) return String(found.id);

	const created = await stripe.post("/v1/billing/meters", {
		display_name: spec.displayName,
		event_name: spec.eventName,
		default_aggregation: { formula: "sum" },
		customer_mapping: {
			type: "by_id",
			event_payload_key: "stripe_customer_id",
		},
		value_settings: { event_payload_key: "value" },
	}, `create meter ${spec.eventName}`);

	return String(created.id);
}

async function ensureProduct(
	stripe: Stripe,
	existing: Json[],
	spec: ProductSpec,
): Promise<string> {
	const found = existing.find((product) =>
		(product.metadata as Json)?.canvist_key === spec.key
	);
	const fields = {
		name: spec.name,
		description: spec.description,
		metadata: { canvist_key: spec.key },
	};

	if (!found) {
		return String(
			(await stripe.post("/v1/products", fields, `create product ${spec.name}`))
				.id,
		);
	}

	if (found.name !== spec.name || found.description !== spec.description) {
		await stripe.post(
			`/v1/products/${found.id}`,
			fields,
			`update product ${spec.name}`,
		);
	}

	return String(found.id);
}

/** The Stripe parameters for `spec`, given the ids it refers to. */
function priceParams(
	spec: PriceSpec,
	product: string,
	meters: Map<string, string>,
): Json {
	const base = {
		product,
		nickname: spec.nickname,
		currency: "usd",
		lookup_key: spec.lookupKey,
	};

	if (spec.licensed) {
		return {
			...base,
			unit_amount: spec.licensed.unitAmount,
			recurring: { interval: spec.licensed.interval },
		};
	}

	const metered = spec.metered!;
	const charged = {
		up_to: "inf",
		unit_amount_decimal: metered.unitAmountDecimal,
	};
	const tiers = metered.included > 0
		? [{ up_to: metered.included, unit_amount: 0 }, charged]
		: [charged];

	return {
		...base,
		billing_scheme: "tiered",
		tiers_mode: "graduated",
		tiers,
		recurring: {
			interval: "month",
			usage_type: "metered",
			meter: meters.get(metered.meter),
		},
	};
}

/**
 * Prices cannot change amounts, so a price whose definition changed is
 * replaced: the new one takes over the lookup key and the old one is
 * archived. `metadata.canvist_spec` records the definition to compare.
 */
async function ensurePrice(
	stripe: Stripe,
	spec: PriceSpec,
	ids: { products: Map<string, string>; meters: Map<string, string> },
): Promise<string> {
	const signature = JSON.stringify({
		licensed: spec.licensed,
		metered: spec.metered,
	});
	const [found] = await stripe.list("/v1/prices", {
		lookup_keys: [spec.lookupKey],
		active: true,
	});

	if (found && (found.metadata as Json)?.canvist_spec === signature) {
		return String(found.id);
	}

	const params = {
		...priceParams(spec, ids.products.get(spec.product)!, ids.meters),
		metadata: { canvist_spec: signature },
		transfer_lookup_key: true,
	};
	const created = await stripe.post(
		"/v1/prices",
		params,
		`create price ${spec.lookupKey}`,
	);

	if (found) {
		await stripe.post(
			`/v1/prices/${found.id}`,
			{ active: false },
			`archive old price ${found.id}`,
		);
	}

	return String(created.id);
}

/** Store a new endpoint's signing secret in Railway without printing it. */
async function storeWebhookSecret(secret: string, environment: string) {
	const command = new Deno.Command("railway", {
		args: [
			"variable",
			"set",
			"STRIPE_WEBHOOK_SECRET",
			"--stdin",
			"--service",
			"sync",
			"--environment",
			environment,
			"--skip-deploys",
		],
		stdin: "piped",
		stdout: "null",
		stderr: "null",
	}).spawn();
	const writer = command.stdin.getWriter();
	await writer.write(new TextEncoder().encode(secret));
	await writer.close();
	const status = await command.status;

	console.log(
		status.success
			? `  stored the signing secret as STRIPE_WEBHOOK_SECRET on sync (${environment})`
			: "  could not store the signing secret in Railway; roll it in the Stripe dashboard and set it by hand",
	);
}

async function ensureWebhook(
	stripe: Stripe,
	options: Options,
): Promise<string> {
	const existing = await stripe.list("/v1/webhook_endpoints");
	const found = existing.find((endpoint) =>
		endpoint.url === options.webhookUrl
	);
	const events = [...WEBHOOK_EVENTS].sort();

	if (found) {
		const current = [...(found.enabled_events as string[])].sort();
		if (current.join() !== events.join()) {
			await stripe.post(`/v1/webhook_endpoints/${found.id}`, {
				enabled_events: events,
			}, "update webhook events");
		}
		return String(found.id);
	}

	const created = await stripe.post("/v1/webhook_endpoints", {
		url: options.webhookUrl,
		enabled_events: events,
		description: "canvist Cloud: subscription changes set project plans",
	}, `create webhook endpoint ${options.webhookUrl}`);

	if (typeof created.secret === "string") {
		await storeWebhookSecret(created.secret, options.railwayEnvironment);
	}

	return String(created.id);
}

async function main() {
	const opts = options();
	const key = await secretKey();
	const stripe = new Stripe(key, { dryRun: opts.dryRun });
	await checkAccount(stripe, key, opts.account);

	const existingMeters = await stripe.list("/v1/billing/meters");
	const meters = new Map<string, string>();
	for (const spec of METERS) {
		meters.set(spec.eventName, await ensureMeter(stripe, existingMeters, spec));
	}

	const existingProducts = await stripe.list("/v1/products", { active: true });
	const products = new Map<string, string>();
	for (const spec of PRODUCTS) {
		products.set(spec.key, await ensureProduct(stripe, existingProducts, spec));
	}

	const prices = new Map<string, string>();
	for (const spec of PRICES) {
		prices.set(
			spec.lookupKey,
			await ensurePrice(stripe, spec, { products, meters }),
		);
	}

	const webhook = await ensureWebhook(stripe, opts);

	console.log(
		JSON.stringify(
			{
				meters: Object.fromEntries(meters),
				products: Object.fromEntries(products),
				prices: Object.fromEntries(prices),
				webhook,
			},
			null,
			2,
		),
	);
}

if (import.meta.main) await main();
