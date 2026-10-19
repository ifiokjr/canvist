# canvist Cloud billing

canvist Cloud bills per monthly active document (MAD) through [Stripe Billing](https://docs.stripe.com/billing), with the provisional plans in [the business model](../v2/business.md). [The Stripe plan](./stripe-plan.md) explains why it is built this way.

Everything here is **test mode only**: the server refuses live keys at startup, the setup script refuses live keys and live accounts, and production has no Stripe keys until the owner approves live billing.

## How it fits together

1. **Plans live on projects.** Each project has a plan (`hobby`, `builder`, `scale`, or `enterprise`), and once it subscribes, a Stripe customer and subscription. Hobby needs nothing from Stripe.
2. **Customers subscribe through Checkout.** `POST /v1/billing/checkout` opens a Checkout Session for a monthly Builder or Scale subscription: the base price plus the MAD and conversion overage prices, with `metadata.canvist_project` naming the project. Every subscription is anchored to 00:00 UTC on the first of the month, so Stripe's billing periods are the same calendar months MADs are counted in. The first partial month's base fee is prorated.
3. **Subscriptions flow back through webhooks.** Stripe sends checkout, subscription, and invoice events to `POST /v1/stripe/webhook`. Each event names a subscription; the server retrieves it and applies its current state, so out-of-order and repeated events are harmless. A paused free-plan session re-checks every 30 seconds, so an upgrade unpauses it without anyone touching it.
4. **Usage flows to Stripe through an outbox.** A document that becomes active on a paid plan is stored with its customer; a conversion on a paid plan is counted as billable. Every 10 seconds the reporter sends what Stripe does not have yet and marks it sent. Each document is one meter event (`mad_<hash of document and month>`); conversions are one event per change in a project's count (`conv_<hash of the range>`). Deploys, crashes, and Stripe outages delay usage; they never lose or double count it.
5. **Customers manage billing themselves.** `POST /v1/billing/plan` switches between Builder and Scale; `POST /v1/billing/portal` opens the Customer Portal for cards, invoices, billing details, tax IDs, and cancellation.

## Payment failure

Losing a paid plan never makes documents read-only.

| Subscription status            | Plan          | What users see                                                                                                   |
| ------------------------------ | ------------- | ---------------------------------------------------------------------------------------------------------------- |
| `active`, `trialing`           | The paid plan | Everything works                                                                                                 |
| `past_due`                     | The paid plan | Everything works while Stripe retries; `GET /v1/usage` shows `past_due`                                          |
| `incomplete`                   | Unchanged     | Checkout has not finished paying                                                                                 |
| `unpaid`, `paused`, `canceled` | Hobby         | Documents already active this month keep syncing; new ones past the free cap pause sync and keep editing locally |

Set Stripe's final dunning action to **mark the subscription unpaid**, so paying the open invoice restores the plan without a new Checkout. An event about an old subscription that no longer pays is ignored, so cancelling a replaced subscription cannot downgrade its replacement.

## The catalog

`services/cloud/stripe/catalog.ts` defines it; `deno task stripe:setup` creates it.

| Object  | Key                                   | Definition                                                         |
| ------- | ------------------------------------- | ------------------------------------------------------------------ |
| Meter   | `canvist_active_document`             | Sum of `value` per `stripe_customer_id`                            |
| Meter   | `canvist_conversion`                  | Sum of `value` per `stripe_customer_id`                            |
| Product | `sync`, `convert`, `history`          | Tax code `txcd_10103001` (SaaS, business use), to be confirmed     |
| Price   | `canvist_builder_monthly` / `_annual` | $29 a month / $288 a year ($24 a month), tax-exclusive             |
| Price   | `canvist_scale_monthly` / `_annual`   | $349 a month / $3,588 a year ($299 a month), tax-exclusive         |
| Price   | `canvist_builder_mad_overage`         | Metered: first 10,000 MADs free, then $2 per 1,000                 |
| Price   | `canvist_scale_mad_overage`           | Metered: first 150,000 MADs free, then $1.50 per 1,000             |
| Price   | `canvist_builder_conversion_overage`  | Metered: $0.01 per conversion                                      |
| Price   | `canvist_scale_conversion_overage`    | Metered: $0.005 per conversion                                     |
| Portal  | configuration `canvist_key=portal`    | Cards, invoices, billing details and tax IDs, cancel at period end |
| Webhook | the staging `sync` URL                | Checkout, subscription, and invoice events                         |

The annual prices exist, but Checkout does not sell them yet: an annual base fee with monthly metered overage is a mixed-interval subscription, which Checkout cannot create. Sell annual plans by hand until the setup-mode Checkout flow exists.

## Tax and invoices

Prices are tax-exclusive and products carry a tax code, but Checkout only asks Stripe Tax to calculate when `STRIPE_AUTOMATIC_TAX=true`. Leave it off until the account has a head office address and an active registration: without them Stripe silently collects nothing. The setup script reports both, without changing them. Checkout always collects tax IDs, so B2B customers abroad get reverse charge where it applies.

## Test-mode objects

Created on 2026-10-19 by `deno task stripe:setup` in the **canvist** account (`acct_1USC6QCmi4vRJir3`), test mode. These are identifiers, not secrets. Live mode has nothing yet.

| Object                                     | ID                                          |
| ------------------------------------------ | ------------------------------------------- |
| Meter `canvist_active_document`            | `mtr_test_61VbaZGHTpFRTycVS41Cmi4vRJir38Rs` |
| Meter `canvist_conversion`                 | `mtr_test_61VbaZGGOosOuZkVf41Cmi4vRJir3H7o` |
| Product canvist Sync                       | `prod_VT8teYZunXzkGC`                       |
| Product canvist Convert                    | `prod_VT8thUxavcrhpi`                       |
| Product canvist History                    | `prod_VT8tRZwHTz7OlV`                       |
| Price `canvist_builder_monthly`            | `price_1USCbwCmi4vRJir3UKuw1bEG`            |
| Price `canvist_builder_annual`             | `price_1USCbwCmi4vRJir3hOJ8CSL3`            |
| Price `canvist_scale_monthly`              | `price_1USCbxCmi4vRJir311LhCDZ0`            |
| Price `canvist_scale_annual`               | `price_1USCbxCmi4vRJir3mATfRBML`            |
| Price `canvist_builder_mad_overage`        | `price_1USCbxCmi4vRJir393ebzBOi`            |
| Price `canvist_scale_mad_overage`          | `price_1USCbyCmi4vRJir3qN7WVG4F`            |
| Price `canvist_builder_conversion_overage` | `price_1USCbyCmi4vRJir3Usp1XzEu`            |
| Price `canvist_scale_conversion_overage`   | `price_1USCbzCmi4vRJir3NschjFVD`            |
| Customer Portal configuration              | `bpc_1USCbzCmi4vRJir34NPfHCBa`              |
| Webhook endpoint (staging `sync`)          | `we_1USCc0Cmi4vRJir3xbMc4WtY`               |

Staging `sync` has `STRIPE_SECRET_KEY` (the Stripe CLI's test key, which expires on 2027-01-17; replace it with a restricted key) and `STRIPE_WEBHOOK_SECRET`. Tax settings are `pending` with no registrations, so `STRIPE_AUTOMATIC_TAX` is off.

## Setting it up

The setup script reads only, unless it has to create or change something, and running it again changes nothing. It finds meters by event name, products and the portal configuration by `metadata.canvist_key`, prices by lookup key, and the webhook endpoint by URL. A price whose definition changed is replaced (the new price takes over the lookup key; the old one is archived), because Stripe prices cannot change their amounts.

```sh
cd services/cloud
stripe login                              # into the canvist account, test mode
deno task stripe:setup --dry-run          # prints the account and what it would do
deno task stripe:setup                    # creates or updates, then prints every id
```

It uses `STRIPE_SECRET_KEY` when set, or else the Stripe CLI's test-mode key. It refuses to run unless the key is a test key, the account is in test mode, and the account's display or business name is "canvist" (or `--account acct_…` names it). `--webhook-url` overrides the endpoint, which defaults to staging's `sync`.

When it creates the webhook endpoint, Stripe returns the endpoint's signing secret once. The script pipes it straight into Railway as `STRIPE_WEBHOOK_SECRET` on `sync` in `staging` (`--railway-environment` to change), never printing it. If that fails, roll the secret in the Stripe dashboard and set it as below.

## Keys in Railway

Set the staging test key on `sync` only, from stdin, so it is never echoed, saved in shell history, or committed. One reporter per database is enough, and `convert` needs no key: its conversions are counted in the shared database.

```sh
railway variable set STRIPE_SECRET_KEY --stdin --service sync --environment staging --skip-deploys
```

Prefer a restricted key (`rk_test_…`) with write access to Billing Meter Events, Checkout Sessions, Customer Portal Sessions, and Subscriptions, and read access to Prices and Customer Portal configurations. The webhook secret works the same way (`STRIPE_WEBHOOK_SECRET`), if the setup script could not store it. Redeploy `sync` afterwards so it reads them.

**Production stays without Stripe keys** until the owner approves live billing. Live billing then needs live keys, which the server refuses today; lifting that check is a deliberate change of its own.
