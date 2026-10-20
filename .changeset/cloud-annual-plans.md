---
"canvist_cloud": minor
---

# Sell annual Builder and Scale plans self-serve

`POST /v1/billing/checkout` now takes `"interval": "year"` (monthly stays the default). An annual plan bills its base fee yearly and its overage monthly, which Stripe Checkout cannot create, so the annual Checkout page saves a card and the Stripe webhook creates the subscription in flexible billing mode, anchored to the first of the month. A retried or repeated webhook never creates a second subscription, and `POST /v1/billing/plan` keeps an annual subscription annual when it switches between Builder and Scale.
