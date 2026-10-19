# Stripe implementation plan

This is the plan for billing canvist Cloud through Stripe. It records what Stripe recommends for a business shaped like canvist, which of those recommendations canvist follows, and where it deliberately does something else. [Billing operations](./billing-operations.md) describes what is built; this page explains why.

> **How this plan was made.** The Stripe plugin's `stripe_implementation_planner` needs the hosted Stripe MCP server, which was not authenticated when this was written. The plan instead follows Stripe's agent best-practices skill (API version `2026-08-26.dahlia`) and the Stripe documentation pages linked below. Run the planner again once the MCP server is connected to the **canvist** account, and reconcile any differences here.

## The business

- **Product.** canvist is an open-source (Unlicense) canvas-rendered rich-text editor. It earns money only from hosted services on Railway: **Sync** (a multi-tenant y-sync relay), **Convert** (JSON, HTML, Markdown, and text conversion), and later **History**.
- **Customers.** Developers and companies who embed the editor. Mostly B2B, worldwide, paying by card.
- **Pricing.** Per monthly active document (MAD), with flat monthly or annual base fees, a free tier with a hard cap, and metered overage. Prices are provisional until the owner approves them ([business model](../business.md)).

| Plan       | Base fee                 | Included MADs | MAD overage     | Conversions |
| ---------- | ------------------------ | ------------- | --------------- | ----------- |
| Hobby      | $0                       | 1,000         | Hard cap        | Counted     |
| Builder    | $29/month ($24 annual)   | 10,000        | $2 per 1,000    | $0.01 each  |
| Scale      | $349/month ($299 annual) | 150,000       | $1.50 per 1,000 | $0.005 each |
| Enterprise | Custom, annual           | Custom        | Negotiated      | Negotiated  |

## Recommended integration

| Need                  | Stripe's recommendation                                                          | canvist's choice                                                                                        |
| --------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Recurring revenue     | [Billing](https://docs.stripe.com/billing) subscriptions, not raw PaymentIntents | Billing subscriptions                                                                                   |
| Customer object       | Customers v1 (Accounts v2 as customers is in public preview)                     | Customers v1, one per paying project                                                                    |
| Usage-based billing   | Metronome for new integrations; Billing Meters for simple pay-as-you-go          | **Billing Meters**: two meters, graduated tiers, no credits or commitments (see below)                  |
| Payment page          | Checkout Sessions in `subscription` mode                                         | Checkout, hosted: `subscription` mode for monthly plans, `setup` mode for annual ones                   |
| Self-serve management | Customer Portal                                                                  | Portal for payment methods, invoices, billing details, tax IDs, and cancellation; plan switches via API |
| Subscription state    | Webhooks for `customer.subscription.*`, `invoice.paid`, `invoice.payment_failed` | All of them, plus `checkout.session.completed`                                                          |
| Tax                   | Stripe Tax with `automatic_tax`, product tax codes, and an active registration   | Tax codes and tax behaviour in the catalog; `automatic_tax` behind a switch until a registration exists |
| B2B invoices          | Tax ID collection, invoice settings, Invoicing for contracts                     | Tax IDs in Checkout and the Portal; Enterprise on `send_invoice`                                        |
| API keys              | Restricted keys (`rk_`) with the fewest permissions                              | A restricted test key on the server; a full test key only for the setup script                          |

### Why Billing Meters and not Metronome

Stripe now routes new usage-based integrations to [Metronome](https://docs.stripe.com/billing/usage-based), which adds enterprise contracts, prepaid credits, and real-time usage views. canvist's self-serve pricing is a flat fee plus graduated overage on two counters, which is the "simple pay-as-you-go on Stripe Billing" case Stripe still serves with [Billing Meters](https://docs.stripe.com/billing/subscriptions/usage-based). canvist also enforces the free cap itself and shows live usage from its own counters (`GET /v1/usage`), so Metronome's real-time views would duplicate what exists. **Revisit for Enterprise**: commitments, ramps, and prepaid credits are where Metronome earns its place.

## Catalog

One product per service, prices for each plan, found by lookup key. Hobby has no Stripe objects: a free project never touches Stripe.

| Object  | Key                                          | Shape                                                                               |
| ------- | -------------------------------------------- | ----------------------------------------------------------------------------------- |
| Meter   | `canvist_active_document`                    | Sum of `value` per `stripe_customer_id`                                             |
| Meter   | `canvist_conversion`                         | Sum of `value` per `stripe_customer_id`                                             |
| Product | `sync`, `convert`, `history`                 | Tax code `txcd_10103001` (SaaS, business use), pending the owner's approval         |
| Price   | `canvist_{builder,scale}_monthly`            | Licensed, monthly, tax-exclusive                                                    |
| Price   | `canvist_{builder,scale}_annual`             | Licensed, yearly, tax-exclusive                                                     |
| Price   | `canvist_{builder,scale}_mad_overage`        | Metered on `canvist_active_document`, graduated: included MADs free                 |
| Price   | `canvist_{builder,scale}_conversion_overage` | Metered on `canvist_conversion`                                                     |
| Portal  | configuration `canvist_key=portal`           | Payment methods, invoice history, billing details and tax IDs, cancel at period end |
| Webhook | the staging `sync` URL                       | The events listed under [Webhooks](#webhooks)                                       |

The tax code is the one Stripe's guidance names for SaaS sold to businesses; confirm it against the [tax code list](https://docs.stripe.com/tax/tax-codes) before taxes are switched on.

## Billing periods follow the calendar month

A MAD is counted per **UTC calendar month**, and a Billing Meter sums events per **subscription billing period**. If the two disagree, a document active on the 20th of one month and the 3rd of the next is counted twice in one billing period, and the included allowance applies to the wrong window.

So every subscription is anchored to the first of the month: Checkout sets `subscription_data.billing_cycle_anchor`, and the server sets `billing_cycle_anchor` on annual subscriptions, to 00:00 UTC on the first of the next month. The base fee for the partial first month is prorated (Stripe's default), and the first partial period gets the full included allowance, which favours the customer.

## Usage reporting

- **Meter Events v1, one per new MAD.** A project's document becomes active at most once a month, so MAD events are sparse. Each carries the identifier `mad_<hash of document and month>`, so a retry inside Stripe's 24-hour uniqueness window never counts twice. v1 validates synchronously, which suits a durable outbox.
- **Conversions are pre-aggregated.** Conversions can arrive in bursts, so the server reports the change in each project's count since the last report as one event (value = the change), as Stripe suggests for high volume. The range is reserved before it is sent, so a retry resends the same value with the same identifier.
- **A durable outbox, not a memory queue.** Usage is written to Postgres first (`active_documents`, `usage`), and a reporter sends what is unreported, marking each row once Stripe accepts it. Deploys, crashes, and Stripe outages delay usage; they do not lose it. Stripe accepts timestamps up to 35 days old, so a backlog of days is safe.
- **Meter Events v2 streams are not needed yet.** They raise the limit to 10,000 events per second in live mode, at the cost of session tokens and asynchronous validation. canvist's volume is far below v1's 1,000 per second.
- **Watch for rejected events.** Stripe reports invalid events asynchronously as `v1.billing.meter.error_report_triggered` thin events. Add an event destination for them in Workbench when going live.

## Checkout, plan changes, and the Portal

- **`POST /v1/billing/checkout`** (API key) creates a Checkout Session for Builder or Scale, billed `"interval": "month"` (the default) or `"year"`. A monthly session is in `subscription` mode: the base price plus both overage prices, `metadata.canvist_project` on the session and the subscription, the month-start anchor, tax ID collection, and `automatic_tax` when enabled. A project that already pays gets `409` and is pointed at the next two endpoints.
- **Annual plans use setup-mode Checkout and the Subscriptions API.** An annual base fee with monthly metered overage is a [mixed-interval subscription](https://docs.stripe.com/billing/subscriptions/mixed-interval), which needs `billing_mode[type]=flexible` and which Checkout cannot create. So a yearly checkout is a `setup` mode session (the price's currency, the customer created or reused, tax ID collection, `metadata.canvist_project` and `metadata.canvist_plan`) that only saves a card. On `checkout.session.completed` for a `setup` session, the server retrieves the session with its SetupIntent expanded and calls `POST /v1/subscriptions`: the annual base price plus both monthly overage prices, flexible billing mode, the month-start anchor, the saved card as `default_payment_method`, `off_session=true`, and `metadata.canvist_project` and `metadata.canvist_checkout_session`. Then it applies the subscription like any other.
- **Creating the annual subscription is idempotent twice over.** The request carries the idempotency key `canvist-annual-<session id>`, so retries within Stripe's 24-hour key window return the first subscription. Stripe retries webhooks for three days, so before creating, the server also lists the customer's subscriptions and applies the one whose `metadata.canvist_checkout_session` names the session. A project that already pays through another subscription gets nothing new: the card is saved, nothing is billed twice.
- **`POST /v1/billing/plan`** (API key) switches between Builder and Scale by swapping each item's price for the same price of the new plan, so an annual subscription stays annual, prorating the base fee. Stripe bills metered usage before the switch at the old plan's rates and starts the new plan's tiers from the switch, which favours the customer either way (tested in test mode on 2026-10-19). The Portal cannot do this: it refuses plan switches on subscriptions with usage-based prices or several products.
- **`POST /v1/billing/portal`** (API key) opens the Customer Portal for payment methods, invoices, billing details, tax IDs, and cancelling at the end of the period.

## Webhooks

The endpoint `POST /v1/stripe/webhook` verifies `Stripe-Signature` against the endpoint secret with a five-minute tolerance, then acts on:

| Event                                                   | What it means for canvist                                                                                                     |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `checkout.session.completed`                            | A project subscribed: link its customer and subscription; for an annual (`setup` mode) session, create the subscription first |
| `customer.subscription.created`, `.updated`, `.deleted` | The plan, status, or items changed                                                                                            |
| `customer.subscription.paused`, `.resumed`              | Collection paused or resumed                                                                                                  |
| `invoice.paid`                                          | A payment recovered a `past_due` subscription                                                                                 |
| `invoice.payment_failed`                                | A payment failed; Stripe is retrying                                                                                          |

Stripe does not deliver events in order, so the handler never trusts the event body's snapshot. Every event names a subscription (or, for an annual checkout, a session whose subscription the server creates), and the handler **retrieves that subscription** and applies its current state. Applying the same state twice changes nothing, so retries and duplicates are harmless.

A project points at one subscription. An event for a different subscription that is no longer paying (an old one, cancelled after an upgrade) is ignored, so it cannot downgrade the project.

## Payment failure never makes the editor read-only

| Subscription status            | Plan the project gets           | What users see                                                                                                     |
| ------------------------------ | ------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `active`, `trialing`           | Its paid plan                   | Everything works                                                                                                   |
| `past_due` (Stripe retrying)   | Its paid plan                   | Everything works; `GET /v1/usage` reports `past_due` so the app can warn                                           |
| `incomplete`                   | Unchanged (Hobby for a new one) | Checkout has not finished paying yet                                                                               |
| `unpaid`, `paused`, `canceled` | Hobby                           | Documents already active this month keep syncing; new ones beyond the free cap pause sync and keep editing locally |

The client never goes read-only: a paused document keeps editing locally and syncs once the project pays again. Configure [Smart Retries](https://docs.stripe.com/billing/revenue-recovery/smart-retries) and set the final action to **mark the subscription unpaid** rather than cancel it, so paying the open invoice restores the plan without a new Checkout.

## Tax and invoices

1. Set the head office address in **Tax settings**. Tax settings stay `pending`, and nothing is calculated, until it is set.
2. Add a registration for each jurisdiction where canvist must collect. Adding one in Stripe records an existing registration; it does not register with the tax authority. Decide where with a tax advisor.
3. Then set `STRIPE_AUTOMATIC_TAX=true`, so Checkout sessions pass `automatic_tax[enabled]=true`. Until a registration is active, `automatic_tax` silently collects nothing, which is why it is off by default.
4. Checkout always collects tax IDs (`tax_id_collection`), and the Portal lets customers maintain them, so cross-border B2B sales get reverse charge where it applies.
5. In **Invoice settings**: an invoice number prefix (for example `CANVIST`), a footer with the legal entity and address, and default payment terms. Enterprise contracts use `collection_method=send_invoice` with net terms.

Threshold monitoring only counts live-mode sales, so its clock starts at the first live payment.

## Keys and secrets

- **Server (`sync`)**: a restricted test key with write access to Billing Meter Events, Checkout Sessions, Customer Portal Sessions, and Subscriptions, and read access to Prices, SetupIntents (for annual checkouts), and Customer Portal configurations. The server refuses live keys outright.
- **Setup script**: a full test-mode key from the Stripe CLI or `STRIPE_SECRET_KEY`. It refuses live keys and any account not named "canvist".
- **Webhook signing secret**: `STRIPE_WEBHOOK_SECRET` on `sync`.
- All of them live only in Railway variables (set from stdin) or Monosecret, never in the repository or logs. Production has none until the owner approves live billing.

## Testing

- Checkout in test mode with Stripe's published card `4242 4242 4242 4242`.
- MAD usage generated against staging Sync, then the meter's event summaries and the upcoming invoice checked for the overage.
- [Test clocks](https://docs.stripe.com/billing/testing/test-clocks) to move a subscription through renewals, failed payments (card `4000 0000 0000 0341`), and recovery, checking that the webhook moves the project's plan.

## What only the owner can do

- Finish the canvist account's business details and activate live mode.
- Approve the prices and the product tax code.
- Set the head office address, add tax registrations, and then enable `STRIPE_AUTOMATIC_TAX`.
- Configure branding, invoice settings, Smart Retries, and the dunning emails.
- Approve live billing, which also means lifting the server's refusal of live keys.
