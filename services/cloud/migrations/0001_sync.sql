-- Projects, API keys, webhooks, and persisted Yjs documents.

CREATE TABLE projects (
	id text PRIMARY KEY,
	name text NOT NULL,
	created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE api_keys (
	hash bytea PRIMARY KEY,
	prefix text NOT NULL,
	project_id text NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
	environment text NOT NULL,
	created_at timestamptz NOT NULL DEFAULT now(),
	revoked_at timestamptz
);

CREATE INDEX api_keys_project ON api_keys (project_id, environment);

CREATE TABLE webhooks (
	project_id text NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
	environment text NOT NULL,
	url text NOT NULL,
	secret text NOT NULL,
	PRIMARY KEY (project_id, environment)
);

-- One row per document. `snapshot` is the full v1 Yjs state as of update
-- `snapshot_seq`; later changes are rows in `document_updates`.
CREATE TABLE documents (
	project_id text NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
	environment text NOT NULL,
	name text NOT NULL,
	snapshot bytea,
	snapshot_seq bigint NOT NULL DEFAULT 0,
	created_at timestamptz NOT NULL DEFAULT now(),
	updated_at timestamptz NOT NULL DEFAULT now(),
	PRIMARY KEY (project_id, environment, name)
);

CREATE TABLE document_updates (
	seq bigserial PRIMARY KEY,
	project_id text NOT NULL,
	environment text NOT NULL,
	name text NOT NULL,
	payload bytea NOT NULL,
	created_at timestamptz NOT NULL DEFAULT now(),
	FOREIGN KEY (project_id, environment, name)
		REFERENCES documents (project_id, environment, name) ON DELETE CASCADE
);

CREATE INDEX document_updates_document ON document_updates (project_id, environment, name, seq);
