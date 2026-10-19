# Plans and billing

Read this page to choose a plan, see what happens when you go over it, and manage your subscription.

**Status: provisional.** These prices aren't final, and billing runs in Stripe's test mode during early access, so nothing is charged yet. Prices are confirmed once real running costs are measured.

## Plans

You pay per **monthly active document** (MAD): a document that someone opened in a sync session, or that your server wrote, during the calendar month (UTC). Each document counts once a month, however often it's used.

| Plan       | Price                     | Included MADs | Overage         |
| ---------- | ------------------------- | ------------- | --------------- |
| Hobby      | $0                        | 1,000         | Hard cap        |
| Builder    | $29/month ($24 annual)    | 10,000        | $2 per 1,000    |
| Scale      | $349/month ($299 annual)  | 150,000       | $1.50 per 1,000 |
| Enterprise | From $2,000/month, annual | Custom        | Negotiated      |

Conversions cost $0.01 each on Builder and $0.005 on Scale. Open-source projects get Builder free, and companies with fewer than five people get 50% off for the first year.

## Going over

- **Hobby has a hard cap that never breaks a client.** Past 1,000 documents, sessions on new documents pause: people keep editing locally, and their work syncs once you upgrade or the month turns. Documents already active this month keep working. See [Sync](./sync.md#when-the-free-plan-is-full).
- **Paid plans never pause.** Past the included documents, you're billed for the overage, per started 1,000.
- **No surprise upgrades.** canvist Cloud warns you at 80% (`standing: "approaching"` in [usage](./api.md#usage)), and never moves you to a bigger plan on its own.

## Subscribe and manage billing

Your app, or your own admin page, drives billing through three routes:

1. **Subscribe:** `POST /v1/billing/checkout` returns a Stripe Checkout page for Builder or Scale. Subscriptions bill on the first of each month; the first partial month's base fee is prorated.
2. **Switch plans:** `POST /v1/billing/plan` moves between Builder and Scale. The base fee is prorated, and usage before the switch is billed at the old plan's rates.
3. **Manage:** `POST /v1/billing/portal` opens the Stripe Customer Portal for cards, invoices, billing details, tax IDs, and cancelling at the end of the period.

Annual plans aren't sold through Checkout yet; ask for one.

## If a payment fails

Losing a paid plan never makes documents read-only.

| Subscription status            | Your plan     | What people see                                                                                     |
| ------------------------------ | ------------- | --------------------------------------------------------------------------------------------------- |
| `active`, `trialing`           | The paid plan | Everything works                                                                                    |
| `past_due`                     | The paid plan | Everything works while Stripe retries the payment                                                   |
| `incomplete`                   | Unchanged     | Checkout hasn't finished paying                                                                     |
| `unpaid`, `paused`, `canceled` | Hobby         | Documents active this month keep syncing; new ones past the free cap pause and keep editing locally |

Paying the open invoice restores the plan, with no new Checkout.

## Self-hosting is free

Running the [same server yourself](./self-hosting.md) costs nothing but your hosting. Set your projects to the `enterprise` plan to remove every cap.
