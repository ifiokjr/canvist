---
"canvist_cloud": minor
---

Count monthly active documents (MADs) and enforce plans.

- A document is active in a month once a sync session opens it or the REST API writes it, counted once per project environment and month in a `usage` table. `GET /v1/usage` reports it against the plan as `ok`, `approaching`, `over`, or `capped`.
- The free plan's cap pauses only sessions on documents new this month: the socket stays open, the client keeps editing locally, documents already active keep syncing, and a paused session syncs on its own after an upgrade. Admissions take a per-project lock, so concurrent sessions cannot overshoot the cap.
- Paid plans warn and never pause. New paid documents send one Stripe Billing Meter event each, test-mode keys only, with deterministic identifiers.
