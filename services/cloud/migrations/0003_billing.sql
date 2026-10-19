-- Stripe subscriptions, and the outbox that reports usage to Stripe.

-- The subscription paying for the project's plan, and its last known status.
ALTER TABLE projects ADD COLUMN stripe_subscription text;
ALTER TABLE projects ADD COLUMN subscription_status text;

-- The customer a document's month is billed to, fixed when it becomes
-- active. NULL on unpaid plans: those documents are never reported.
ALTER TABLE active_documents ADD COLUMN stripe_customer text;
-- When Stripe accepted the meter event.
ALTER TABLE active_documents ADD COLUMN reported_at timestamptz;
CREATE INDEX active_documents_unreported ON active_documents (first_active_at)
	WHERE stripe_customer IS NOT NULL AND reported_at IS NULL;

-- Conversions run on a paid plan, how many Stripe has, and the total being
-- reported right now (so a retry resends the same amount).
ALTER TABLE usage ADD COLUMN billable_conversions bigint NOT NULL DEFAULT 0;
ALTER TABLE usage ADD COLUMN reported_conversions bigint NOT NULL DEFAULT 0;
ALTER TABLE usage ADD COLUMN reserved_conversions bigint;
