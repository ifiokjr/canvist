//! The Postgres store.
//!
//! Connections come from a small `deadpool` pool. Railway's Postgres is
//! reached over its private network, so connections are unencrypted; put a
//! TLS-terminating proxy in front when the database is elsewhere.

use std::str::FromStr;

use deadpool_postgres::Manager;
use deadpool_postgres::ManagerConfig;
use deadpool_postgres::Pool;
use deadpool_postgres::RecyclingMethod;
use tokio_postgres::NoTls;

use super::DocumentSummary;
use super::StoreError;
use super::StoredDocument;
use super::Webhook;
use crate::auth::KeyHash;
use crate::auth::NewApiKey;
use crate::names::DocumentKey;
use crate::names::DocumentName;
use crate::names::Environment;
use crate::names::ProjectId;

/// Every migration, in order. Each runs once, inside a transaction.
const MIGRATIONS: &[(i32, &str)] = &[(1, include_str!("../../migrations/0001_sync.sql"))];

/// Connections per instance. Railway's smallest Postgres allows 100, so
/// several replicas fit.
const POOL_SIZE: usize = 16;

/// An arbitrary, fixed key for the advisory lock that serializes
/// migrations across replicas.
const MIGRATION_LOCK: i64 = 0x0063_616e_7669_7374;

/// A pool of Postgres connections.
#[derive(Clone)]
pub struct PgStore {
	pool: Pool,
}

impl std::fmt::Debug for PgStore {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.debug_struct("PgStore").finish_non_exhaustive()
	}
}

impl PgStore {
	/// Connect to `url` and migrate.
	pub async fn connect(url: &str) -> Result<Self, StoreError> {
		let config = tokio_postgres::Config::from_str(url)?;
		let manager = Manager::from_config(
			config,
			NoTls,
			ManagerConfig {
				recycling_method: RecyclingMethod::Fast,
			},
		);
		let pool = Pool::builder(manager)
			.max_size(POOL_SIZE)
			.build()
			.map_err(|error| StoreError::Config(error.to_string()))?;
		let store = Self { pool };
		store.migrate().await?;
		Ok(store)
	}

	async fn migrate(&self) -> Result<(), StoreError> {
		let mut client = self.pool.get().await?;
		let transaction = client.transaction().await?;
		// Replicas starting together take turns; the lock is released when
		// the transaction ends.
		transaction
			.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK])
			.await?;
		transaction
			.batch_execute(
				"CREATE TABLE IF NOT EXISTS canvist_migrations (
					version integer PRIMARY KEY,
					applied_at timestamptz NOT NULL DEFAULT now()
				)",
			)
			.await?;

		for (version, sql) in MIGRATIONS {
			let applied = transaction
				.query_opt(
					"SELECT 1 FROM canvist_migrations WHERE version = $1",
					&[version],
				)
				.await?
				.is_some();

			if !applied {
				tracing::info!(version, "applying migration");
				transaction.batch_execute(sql).await?;
				transaction
					.execute(
						"INSERT INTO canvist_migrations (version) VALUES ($1)",
						&[version],
					)
					.await?;
			}
		}

		transaction.commit().await?;
		Ok(())
	}

	pub(super) async fn ping(&self) -> Result<(), StoreError> {
		self.pool.get().await?.execute("SELECT 1", &[]).await?;
		Ok(())
	}

	pub(super) async fn create_project(
		&self,
		id: &ProjectId,
		name: &str,
	) -> Result<(), StoreError> {
		self.pool
			.get()
			.await?
			.execute(
				"INSERT INTO projects (id, name) VALUES ($1, $2)",
				&[&id.as_str(), &name],
			)
			.await?;
		Ok(())
	}

	pub(super) async fn project_exists(&self, id: &ProjectId) -> Result<bool, StoreError> {
		let row = self
			.pool
			.get()
			.await?
			.query_opt("SELECT 1 FROM projects WHERE id = $1", &[&id.as_str()])
			.await?;
		Ok(row.is_some())
	}

	pub(super) async fn add_api_key(
		&self,
		project: &ProjectId,
		environment: &Environment,
		key: &NewApiKey,
	) -> Result<(), StoreError> {
		if !self.project_exists(project).await? {
			return Err(StoreError::UnknownProject);
		}

		self.pool
			.get()
			.await?
			.execute(
				"INSERT INTO api_keys (hash, prefix, project_id, environment) VALUES ($1, $2, $3, \
				 $4)",
				&[
					&key.hash.as_slice(),
					&key.prefix,
					&project.as_str(),
					&environment.as_str(),
				],
			)
			.await?;
		Ok(())
	}

	pub(super) async fn find_api_key(
		&self,
		hash: &KeyHash,
	) -> Result<Option<(ProjectId, Environment)>, StoreError> {
		let row = self
			.pool
			.get()
			.await?
			.query_opt(
				"SELECT project_id, environment FROM api_keys WHERE hash = $1 AND revoked_at IS \
				 NULL",
				&[&hash.as_slice()],
			)
			.await?;

		Ok(row.and_then(|row| {
			let project: String = row.get(0);
			let environment: String = row.get(1);
			Some((
				ProjectId::new(project),
				Environment::parse(&environment).ok()?,
			))
		}))
	}

	pub(super) async fn set_webhook(
		&self,
		project: &ProjectId,
		environment: &Environment,
		webhook: Option<&Webhook>,
	) -> Result<(), StoreError> {
		let client = self.pool.get().await?;
		let ids: [&(dyn tokio_postgres::types::ToSql + Sync); 2] =
			[&project.as_str(), &environment.as_str()];

		match webhook {
			Some(webhook) => {
				client
					.execute(
						"INSERT INTO webhooks (project_id, environment, url, secret)
						VALUES ($1, $2, $3, $4)
						ON CONFLICT (project_id, environment)
						DO UPDATE SET url = EXCLUDED.url, secret = EXCLUDED.secret",
						&[ids[0], ids[1], &webhook.url, &webhook.secret],
					)
					.await?;
			}
			None => {
				client
					.execute(
						"DELETE FROM webhooks WHERE project_id = $1 AND environment = $2",
						&ids,
					)
					.await?;
			}
		}

		Ok(())
	}

	pub(super) async fn webhook(
		&self,
		project: &ProjectId,
		environment: &Environment,
	) -> Result<Option<Webhook>, StoreError> {
		let row = self
			.pool
			.get()
			.await?
			.query_opt(
				"SELECT url, secret FROM webhooks WHERE project_id = $1 AND environment = $2",
				&[&project.as_str(), &environment.as_str()],
			)
			.await?;

		Ok(row.map(|row| {
			Webhook {
				url: row.get(0),
				secret: row.get(1),
			}
		}))
	}

	pub(super) async fn load(&self, key: &DocumentKey) -> Result<StoredDocument, StoreError> {
		let client = self.pool.get().await?;
		let ids = document_ids(key);
		let Some(row) = client
			.query_opt(
				"SELECT snapshot, snapshot_seq FROM documents
				WHERE project_id = $1 AND environment = $2 AND name = $3",
				&ids.params(),
			)
			.await?
		else {
			return Ok(StoredDocument::default());
		};
		let snapshot: Option<Vec<u8>> = row.get(0);
		let snapshot_seq: i64 = row.get(1);

		let rows = client
			.query(
				"SELECT seq, payload FROM document_updates
				WHERE project_id = $1 AND environment = $2 AND name = $3
				ORDER BY seq",
				&ids.params(),
			)
			.await?;
		let last_seq = rows.last().map_or(snapshot_seq, |row| row.get(0));

		Ok(StoredDocument {
			snapshot,
			updates: rows.into_iter().map(|row| row.get(1)).collect(),
			last_seq,
		})
	}

	pub(super) async fn append(&self, key: &DocumentKey, update: &[u8]) -> Result<i64, StoreError> {
		let mut client = self.pool.get().await?;
		let ids = document_ids(key);
		let transaction = client.transaction().await?;
		transaction
			.execute(
				"INSERT INTO documents (project_id, environment, name) VALUES ($1, $2, $3)
				ON CONFLICT DO NOTHING",
				&ids.params(),
			)
			.await?;
		let row = transaction
			.query_one(
				"INSERT INTO document_updates (project_id, environment, name, payload)
				VALUES ($1, $2, $3, $4) RETURNING seq",
				&[&ids.project, &ids.environment, &ids.name, &update],
			)
			.await?;
		transaction
			.execute(
				"UPDATE documents SET updated_at = now()
				WHERE project_id = $1 AND environment = $2 AND name = $3",
				&ids.params(),
			)
			.await?;
		transaction.commit().await?;
		Ok(row.get(0))
	}

	pub(super) async fn compact(
		&self,
		key: &DocumentKey,
		snapshot: &[u8],
		through_seq: i64,
	) -> Result<(), StoreError> {
		let mut client = self.pool.get().await?;
		let ids = document_ids(key);
		let transaction = client.transaction().await?;
		transaction
			.execute(
				"INSERT INTO documents (project_id, environment, name, snapshot, snapshot_seq)
				VALUES ($1, $2, $3, $4, $5)
				ON CONFLICT (project_id, environment, name)
				DO UPDATE SET snapshot = EXCLUDED.snapshot, snapshot_seq = EXCLUDED.snapshot_seq,
					updated_at = now()",
				&[
					&ids.project,
					&ids.environment,
					&ids.name,
					&snapshot,
					&through_seq,
				],
			)
			.await?;
		transaction
			.execute(
				"DELETE FROM document_updates
				WHERE project_id = $1 AND environment = $2 AND name = $3 AND seq <= $4",
				&[&ids.project, &ids.environment, &ids.name, &through_seq],
			)
			.await?;
		transaction.commit().await?;
		Ok(())
	}

	pub(super) async fn list(
		&self,
		project: &ProjectId,
		environment: &Environment,
	) -> Result<Vec<DocumentSummary>, StoreError> {
		let rows = self
			.pool
			.get()
			.await?
			.query(
				"SELECT d.name, count(u.seq) FROM documents d
				LEFT JOIN document_updates u USING (project_id, environment, name)
				WHERE d.project_id = $1 AND d.environment = $2
				GROUP BY d.name ORDER BY d.name",
				&[&project.as_str(), &environment.as_str()],
			)
			.await?;

		Ok(rows
			.into_iter()
			.filter_map(|row| {
				let name: String = row.get(0);
				Some(DocumentSummary {
					name: DocumentName::parse(&name).ok()?,
					pending_updates: row.get(1),
				})
			})
			.collect())
	}

	pub(super) async fn delete(&self, key: &DocumentKey) -> Result<bool, StoreError> {
		let deleted = self
			.pool
			.get()
			.await?
			.execute(
				"DELETE FROM documents WHERE project_id = $1 AND environment = $2 AND name = $3",
				&document_ids(key).params(),
			)
			.await?;
		Ok(deleted > 0)
	}
}

/// A document key as query parameters.
struct DocumentIds<'a> {
	project: &'a str,
	environment: &'a str,
	name: &'a str,
}

impl DocumentIds<'_> {
	fn params(&self) -> [&(dyn tokio_postgres::types::ToSql + Sync); 3] {
		[&self.project, &self.environment, &self.name]
	}
}

fn document_ids(key: &DocumentKey) -> DocumentIds<'_> {
	DocumentIds {
		project: key.project.as_str(),
		environment: key.environment.as_str(),
		name: key.name.as_str(),
	}
}
