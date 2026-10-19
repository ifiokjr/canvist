-- Plans, monthly active documents, and usage counters.

ALTER TABLE projects ADD COLUMN plan text NOT NULL DEFAULT 'hobby';
-- Overrides the plan's included documents, as for enterprise contracts.
ALTER TABLE projects ADD COLUMN included_documents bigint;
-- The Stripe customer that metered usage is reported for.
ALTER TABLE projects ADD COLUMN stripe_customer text;

-- One row per document active in a month: the deduplication behind MADs.
CREATE TABLE active_documents (
	project_id text NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
	environment text NOT NULL,
	month text NOT NULL,
	name text NOT NULL,
	first_active_at timestamptz NOT NULL DEFAULT now(),
	PRIMARY KEY (project_id, environment, month, name)
);

-- Counters per project environment and month (`YYYY-MM`, UTC).
CREATE TABLE usage (
	project_id text NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
	environment text NOT NULL,
	month text NOT NULL,
	active_documents bigint NOT NULL DEFAULT 0,
	conversions bigint NOT NULL DEFAULT 0,
	PRIMARY KEY (project_id, environment, month)
);
