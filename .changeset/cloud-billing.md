---
"canvist_cloud": minor
---

Bill canvist Cloud the way Stripe recommends. Usage is reported through a durable Postgres outbox, so restarts and Stripe outages delay it without losing it, and conversions are now billed on the `canvist_conversion` meter as aggregated counts. Subscriptions start with `POST /v1/billing/checkout`, anchored to the first of the month so Stripe's billing periods match monthly active documents; `POST /v1/billing/plan` switches between Builder and Scale; and `POST /v1/billing/portal` opens the Customer Portal. The webhook now handles checkout and invoice events and applies each subscription's current state, so out-of-order events and old subscriptions cannot change a project's plan, and a failed payment keeps the plan while Stripe retries. The setup script adds product tax codes, tax-exclusive prices, the Customer Portal configuration, and a Stripe Tax readiness check. See `docs/src/cloud/stripe-plan.md`.
