---
"canvist_cloud": minor
---

Close the billing loop with Stripe: `POST /v1/stripe/webhook` verifies Stripe's signature and turns subscription events into project plans, so an upgrade unpauses a full free plan on its own. Add `services/cloud/stripe/setup.ts` (`deno task stripe:setup`), an idempotent, test-mode-only script that creates the Billing Meters, products, prices, and webhook endpoint for the provisional plans, and document billing in `docs/src/cloud/billing.md`.
