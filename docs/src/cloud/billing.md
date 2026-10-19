# canvist Cloud billing

canvist Cloud bills per monthly active document (MAD) through [Stripe Billing](https://docs.stripe.com/billing), with the provisional plans in [the business model](../v2/business.md). Everything here is **test mode only**: the server refuses live keys at startup, the setup script refuses live keys and live accounts, and production has no Stripe keys until the owner approves live billing.

## How it fits together

1. **Plans live on projects.** Each project has a plan (`hobby`, `builder`, `scale`, or `enterprise`) and, once it pays, a Stripe customer.
2. **Usage flows to Stripe.** When a document becomes active in a month on a project with a Stripe customer, the server sends one meter event to the `canvist_active_document` Billing Meter, with a deterministic identifier so retries never double count.
3. **Subscriptions flow back.** Stripe sends `customer.subscription.created`, `updated`, and `deleted` to `POST /v1/stripe/webhook`. The subscription names its project in `metadata.canvist_project`, and its plan through a price's lookup key (`canvist_builder_…`, `canvist_scale_…`). An active, trialing, or past-due subscription sets that plan; anything else returns the project to Hobby. Because paused sessions re-check every 30 seconds, an upgrade unpauses a full free plan's documents without anyone touching them.

## The catalog

`services/cloud/stripe/catalog.ts` defines it; `deno task stripe:setup` creates it.

| Object  | Key                                   | Definition                                              |
| ------- | ------------------------------------- | ------------------------------------------------------- |
| Meter   | `canvist_active_document`             | Sum of `value` per `stripe_customer_id`                 |
| Meter   | `canvist_conversion`                  | Sum of `value` per `stripe_customer_id`                 |
| Product | `sync`, `convert`, `history`          | canvist Sync, canvist Convert, canvist History          |
| Price   | `canvist_hobby_monthly`               | $0 a month                                              |
| Price   | `canvist_builder_monthly` / `_annual` | $29 a month / $288 a year ($24 a month)                 |
| Price   | `canvist_scale_monthly` / `_annual`   | $349 a month / $3,588 a year ($299 a month)             |
| Price   | `canvist_builder_mad_overage`         | Metered: first 10,000 MADs free, then $2 per 1,000      |
| Price   | `canvist_scale_mad_overage`           | Metered: first 150,000 MADs free, then $1.50 per 1,000  |
| Price   | `canvist_builder_conversion_overage`  | Metered: $0.01 per conversion                           |
| Price   | `canvist_scale_conversion_overage`    | Metered: $0.005 per conversion                          |
| Webhook | the staging `sync` URL                | `customer.subscription.created`, `.updated`, `.deleted` |

A Builder subscription combines `canvist_builder_monthly` with `canvist_builder_mad_overage` (and the conversion overage), with `metadata.canvist_project` set to the project id. Annual plans bill yearly while the overage meters monthly; Stripe needs [flexible billing mode](https://docs.stripe.com/billing/subscriptions/billing-mode) to combine the two in one subscription.

The server does not report conversions to Stripe yet; they are counted in `GET /v1/usage`, and the `canvist_conversion` meter is ready for when they are.

## Setting it up

The setup script reads only, unless it has to create or change something, and running it again changes nothing. It finds meters by event name, products by `metadata.canvist_key`, prices by lookup key, and the webhook endpoint by URL. A price whose definition changed is replaced (the new price takes over the lookup key; the old one is archived), because Stripe prices cannot change their amounts.

```sh
cd services/cloud
stripe login                              # into the canvist account, test mode
deno task stripe:setup --dry-run          # prints the account and what it would do
deno task stripe:setup                    # creates or updates, then prints every id
```

It uses `STRIPE_SECRET_KEY` when set, or else the Stripe CLI's test-mode key. It refuses to run unless the key is a test key, the account is in test mode, and the account's display or business name is "canvist" (or `--account acct_…` names it). `--webhook-url` overrides the endpoint, which defaults to staging's `sync`.

When it creates the webhook endpoint, Stripe returns the endpoint's signing secret once. The script pipes it straight into Railway as `STRIPE_WEBHOOK_SECRET` on `sync` in `staging` (`--railway-environment` to change), never printing it. If that fails, roll the secret in the Stripe dashboard and set it as below.

## Keys in Railway

Set the staging test key on `sync` from stdin, so it is never echoed, saved in shell history, or committed:

```sh
railway variable set STRIPE_SECRET_KEY --stdin --service sync --environment staging --skip-deploys
```

Paste the `sk_test_…` key, press Enter, then Ctrl-D. The webhook secret works the same way (`STRIPE_WEBHOOK_SECRET`), if the setup script could not store it. Redeploy `sync` afterwards so it reads them.

**Production stays without Stripe keys** until the owner approves live billing. Live billing then needs live keys, which the server refuses today; lifting that check is a deliberate change of its own.
