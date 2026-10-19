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

use super::Admit;
use super::DocumentSummary;
use super::PendingConversions;
use super::PendingDocument;
use super::StoreError;
use super::StoredDocument;
use super::UsageRow;
use super::Webhook;
use crate::auth::KeyHash;
use crate::auth::NewApiKey;
use crate::metering::Month;
use crate::metering::Plan;
use crate::metering::Subscription;
use crate::metering::Terms;
use crate::names::DocumentKey;
use crate::names::DocumentName;
use crate::names::Environment;
use crate::names::ProjectId;

/// Every migration, in order. Each runs once, inside a transaction.
const MIGRATIONS: &[(i32, &str)] = &[
	(1, include_str!("../../migrations/0001_sync.sql")),
	(2, include_str!("../../migrations/0002_metering.sql")),
	(3, include_str!("../../migrations/0003_billing.sql")),
];

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

	pub(super) async fn terms(&self, project: &ProjectId) -> Result<Terms, StoreError> {
		let client = self.pool.get().await?;
		read_terms(&**client, project).await
	}

	pub(super) async fn set_terms(
		&self,
		project: &ProjectId,
		terms: &Terms,
	) -> Result<(), StoreError> {
		// Only an override is stored, so a later plan change takes the new
		// plan's default.
		let included = terms
			.included_documents
			.filter(|included| Some(*included) != terms.plan.included_documents())
			.map(|included| i64::try_from(included).unwrap_or(i64::MAX));
		let subscription = terms
			.subscription
			.as_ref()
			.map(|subscription| subscription.id.as_str());
		let status = terms
			.subscription
			.as_ref()
			.map(|subscription| subscription.status.as_str());
		let updated = self
			.pool
			.get()
			.await?
			.execute(
				"UPDATE projects SET plan = $2, included_documents = $3, stripe_customer = $4,
					stripe_subscription = $5, subscription_status = $6
				WHERE id = $1",
				&[
					&project.as_str(),
					&terms.plan.as_str(),
					&included,
					&terms.stripe_customer,
					&subscription,
					&status,
				],
			)
			.await?;

		if updated == 0 {
			return Err(StoreError::UnknownProject);
		}

		Ok(())
	}

	/// The admission transaction: documents already active this month pass
	/// without a lock; new ones take the project's month lock, so two
	/// documents admitted at once cannot both take a free plan's last slot.
	pub(super) async fn admit(&self, key: &DocumentKey, month: Month) -> Result<Admit, StoreError> {
		let mut client = self.pool.get().await?;
		let transaction = client.transaction().await?;
		let terms = read_terms(&*transaction, &key.project).await?;
		let month = month.to_string();
		let ids = document_ids(key);
		let row: [&(dyn tokio_postgres::types::ToSql + Sync); 4] =
			[&ids.project, &ids.environment, &month, &ids.name];
		let active_already = transaction
			.query_opt(
				"SELECT 1 FROM active_documents
				WHERE project_id = $1 AND environment = $2 AND month = $3 AND name = $4",
				&row,
			)
			.await?
			.is_some();

		if active_already {
			transaction.commit().await?;
			return Ok(Admit {
				admitted: true,
				new: false,
				active: None,
				terms,
			});
		}

		transaction
			.execute(
				"SELECT pg_advisory_xact_lock(hashtextextended($1 || '/' || $2, 0))",
				&[&ids.project, &month],
			)
			.await?;
		let active: i64 = transaction
			.query_one(
				"SELECT coalesce(sum(active_documents), 0)::bigint FROM usage
				WHERE project_id = $1 AND month = $2",
				&[&ids.project, &month],
			)
			.await?
			.get(0);
		let active = u64::try_from(active).unwrap_or_default();

		if terms.is_full(active) {
			transaction.commit().await?;
			return Ok(Admit {
				admitted: false,
				new: false,
				active: Some(active),
				terms,
			});
		}

		let customer = terms.metered_customer();
		let inserted = transaction
			.execute(
				"INSERT INTO active_documents (project_id, environment, month, name, \
				 stripe_customer)
				VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
				&[row[0], row[1], row[2], row[3], &customer],
			)
			.await?
			== 1;

		if inserted {
			transaction
				.execute(
					"INSERT INTO usage (project_id, environment, month, active_documents)
					VALUES ($1, $2, $3, 1)
					ON CONFLICT (project_id, environment, month)
					DO UPDATE SET active_documents = usage.active_documents + 1",
					&[&ids.project, &ids.environment, &month],
				)
				.await?;
		}

		transaction.commit().await?;

		Ok(Admit {
			admitted: true,
			new: inserted,
			active: Some(active + u64::from(inserted)),
			terms,
		})
	}

	pub(super) async fn add_conversions(
		&self,
		project: &ProjectId,
		environment: &Environment,
		month: Month,
		count: u64,
	) -> Result<(), StoreError> {
		let count = i64::try_from(count).unwrap_or(i64::MAX);
		// Billable when the project meters usage, as `Terms::metered_customer`
		// decides.
		self.pool
			.get()
			.await?
			.execute(
				"INSERT INTO usage (project_id, environment, month, conversions, billable_conversions)
				SELECT id, $2::text, $3::text, $4::bigint,
					CASE WHEN plan <> 'hobby' AND stripe_customer IS NOT NULL THEN $4::bigint ELSE 0 END
				FROM projects WHERE id = $1
				ON CONFLICT (project_id, environment, month)
				DO UPDATE SET conversions = usage.conversions + EXCLUDED.conversions,
					billable_conversions = usage.billable_conversions + EXCLUDED.billable_conversions",
				&[
					&project.as_str(),
					&environment.as_str(),
					&month.to_string(),
					&count,
				],
			)
			.await?;
		Ok(())
	}

	pub(super) async fn pending_documents(
		&self,
		limit: usize,
	) -> Result<Vec<PendingDocument>, StoreError> {
		let limit = i64::try_from(limit).unwrap_or(i64::MAX);
		let rows = self
			.pool
			.get()
			.await?
			.query(
				"SELECT project_id, environment, month, name, stripe_customer,
					extract(epoch FROM first_active_at)::bigint
				FROM active_documents
				WHERE stripe_customer IS NOT NULL AND reported_at IS NULL
				ORDER BY first_active_at LIMIT $1",
				&[&limit],
			)
			.await?;

		Ok(rows
			.into_iter()
			.filter_map(|row| {
				let environment: String = row.get(1);
				let month: String = row.get(2);
				let name: String = row.get(3);
				Some(PendingDocument {
					key: DocumentKey {
						project: ProjectId::new(row.get::<_, String>(0)),
						environment: Environment::parse(&environment).ok()?,
						name: DocumentName::parse(&name).ok()?,
					},
					month: month.parse().ok()?,
					customer: row.get(4),
					first_active_at: row.get(5),
				})
			})
			.collect())
	}

	pub(super) async fn mark_document_reported(
		&self,
		key: &DocumentKey,
		month: Month,
	) -> Result<(), StoreError> {
		let ids = document_ids(key);
		self.pool
			.get()
			.await?
			.execute(
				"UPDATE active_documents SET reported_at = now()
				WHERE project_id = $1 AND environment = $2 AND month = $3 AND name = $4",
				&[
					&ids.project,
					&ids.environment,
					&month.to_string(),
					&ids.name,
				],
			)
			.await?;
		Ok(())
	}

	pub(super) async fn reserve_conversions(
		&self,
		limit: usize,
	) -> Result<Vec<PendingConversions>, StoreError> {
		let limit = i64::try_from(limit).unwrap_or(i64::MAX);
		let client = self.pool.get().await?;
		client
			.execute(
				"UPDATE usage SET reserved_conversions = billable_conversions
				WHERE (project_id, environment, month) IN (
					SELECT project_id, environment, month FROM usage
					WHERE reserved_conversions IS NULL
						AND billable_conversions > reported_conversions
					LIMIT $1
				)",
				&[&limit],
			)
			.await?;
		let rows = client
			.query(
				"SELECT usage.project_id, usage.environment, usage.month, \
				 projects.stripe_customer,
					usage.reported_conversions, usage.reserved_conversions
				FROM usage JOIN projects ON projects.id = usage.project_id
				WHERE usage.reserved_conversions IS NOT NULL
					AND projects.stripe_customer IS NOT NULL
				LIMIT $1",
				&[&limit],
			)
			.await?;

		Ok(rows
			.into_iter()
			.filter_map(|row| {
				let environment: String = row.get(1);
				let month: String = row.get(2);
				let from: i64 = row.get(4);
				let to: i64 = row.get(5);
				Some(PendingConversions {
					project: ProjectId::new(row.get::<_, String>(0)),
					environment: Environment::parse(&environment).ok()?,
					month: month.parse().ok()?,
					customer: row.get(3),
					from: u64::try_from(from).ok()?,
					to: u64::try_from(to).ok()?,
				})
			})
			.collect())
	}

	pub(super) async fn mark_conversions_reported(
		&self,
		pending: &PendingConversions,
	) -> Result<(), StoreError> {
		let to = i64::try_from(pending.to).unwrap_or(i64::MAX);
		self.pool
			.get()
			.await?
			.execute(
				"UPDATE usage SET reported_conversions = $4, reserved_conversions = NULL
				WHERE project_id = $1 AND environment = $2 AND month = $3
					AND reserved_conversions = $4",
				&[
					&pending.project.as_str(),
					&pending.environment.as_str(),
					&pending.month.to_string(),
					&to,
				],
			)
			.await?;
		Ok(())
	}

	pub(super) async fn usage(
		&self,
		project: &ProjectId,
		month: Month,
	) -> Result<Vec<UsageRow>, StoreError> {
		let rows = self
			.pool
			.get()
			.await?
			.query(
				"SELECT environment, active_documents, conversions FROM usage
				WHERE project_id = $1 AND month = $2 ORDER BY environment",
				&[&project.as_str(), &month.to_string()],
			)
			.await?;

		Ok(rows
			.into_iter()
			.filter_map(|row| {
				let environment: String = row.get(0);
				let active: i64 = row.get(1);
				let conversions: i64 = row.get(2);
				Some(UsageRow {
					environment: Environment::parse(&environment).ok()?,
					active_documents: u64::try_from(active).ok()?,
					conversions: u64::try_from(conversions).ok()?,
				})
			})
			.collect())
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

/// `project`'s terms, read through any client or transaction.
async fn read_terms(
	client: &impl tokio_postgres::GenericClient,
	project: &ProjectId,
) -> Result<Terms, StoreError> {
	let row = client
		.query_opt(
			"SELECT plan, included_documents, stripe_customer, stripe_subscription,
				subscription_status
			FROM projects WHERE id = $1",
			&[&project.as_str()],
		)
		.await?
		.ok_or(StoreError::UnknownProject)?;
	let plan: String = row.get(0);
	let included: Option<i64> = row.get(1);
	let plan: Plan = plan.parse().unwrap_or(Plan::Hobby);
	let subscription: Option<String> = row.get(3);
	let status: Option<String> = row.get(4);

	Ok(Terms {
		plan,
		included_documents: included
			.and_then(|included| u64::try_from(included).ok())
			.or_else(|| plan.included_documents()),
		stripe_customer: row.get(2),
		subscription: subscription.zip(status).and_then(|(id, status)| {
			Some(Subscription {
				id,
				status: status.parse().ok()?,
			})
		}),
	})
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
