//! Durable state: projects, keys, webhooks, and documents.
//!
//! [`Store`] is Postgres in production and an in-memory map for tests and
//! quick local runs. Documents are stored the way y-websocket persistence
//! adapters store them: a snapshot (the full Yjs state) plus the updates
//! appended since, which [`Store::compact`] folds back into the snapshot.

mod memory;
mod postgres;

use std::sync::Arc;

pub use memory::MemoryStore;
pub use postgres::PgStore;

use crate::auth::KeyHash;
use crate::auth::NewApiKey;
use crate::metering::Month;
use crate::metering::Terms;
use crate::names::DocumentKey;
use crate::names::DocumentName;
use crate::names::Environment;
use crate::names::ProjectId;

/// A failure talking to the store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
	/// Postgres refused a query.
	#[error("database error: {0}")]
	Postgres(#[from] tokio_postgres::Error),
	/// No connection could be had from the pool.
	#[error("database pool error: {0}")]
	Pool(#[from] deadpool_postgres::PoolError),
	/// The pool could not be built.
	#[error("database configuration error: {0}")]
	Config(String),
	/// The referenced project does not exist.
	#[error("unknown project")]
	UnknownProject,
}

/// Where a webhook goes and how its body is signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Webhook {
	/// The HTTPS endpoint to POST to.
	pub url: String,
	/// The HMAC-SHA256 key for the `Canvist-Signature` header.
	pub secret: String,
}

/// A document as stored: a snapshot plus the updates since.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoredDocument {
	/// The compacted v1 state, if the document was ever compacted.
	pub snapshot: Option<Vec<u8>>,
	/// v1 updates appended after the snapshot, oldest first.
	pub updates: Vec<Vec<u8>>,
	/// The sequence number of the newest update, or of the snapshot.
	pub last_seq: i64,
}

impl StoredDocument {
	/// Whether nothing was ever stored.
	pub fn is_empty(&self) -> bool {
		self.snapshot.is_none() && self.updates.is_empty()
	}
}

/// A stored document's name and size, for listings.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DocumentSummary {
	/// The document name.
	pub name: DocumentName,
	/// Update rows waiting for compaction.
	pub pending_updates: i64,
}

/// The outcome of admitting a document for a month.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admit {
	/// Whether the document may sync. Only a full free plan refuses, and
	/// only documents not yet active this month.
	pub admitted: bool,
	/// Whether this made the document active this month: a new MAD.
	pub new: bool,
	/// The project's active documents this month, when it was counted.
	pub active: Option<u64>,
	/// The project's billing terms.
	pub terms: Terms,
}

/// One environment's usage in a month.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct UsageRow {
	/// The environment.
	pub environment: Environment,
	/// Documents active this month.
	pub active_documents: u64,
	/// Conversions run this month.
	pub conversions: u64,
}

/// The durable store.
#[derive(Clone, Debug)]
pub enum Store {
	/// Process memory: lost on restart.
	Memory(Arc<MemoryStore>),
	/// Postgres.
	Postgres(PgStore),
}

impl Store {
	/// An empty in-memory store.
	pub fn memory() -> Self {
		Self::Memory(Arc::default())
	}

	/// Connect to Postgres at `url` and run pending migrations.
	pub async fn postgres(url: &str) -> Result<Self, StoreError> {
		Ok(Self::Postgres(PgStore::connect(url).await?))
	}

	/// Whether the store answers.
	pub async fn ping(&self) -> Result<(), StoreError> {
		match self {
			Self::Memory(_) => Ok(()),
			Self::Postgres(store) => store.ping().await,
		}
	}

	/// Create a project named `name`.
	pub async fn create_project(&self, name: &str) -> Result<ProjectId, StoreError> {
		let id = ProjectId::generate();

		match self {
			Self::Memory(store) => store.create_project(&id, name),
			Self::Postgres(store) => store.create_project(&id, name).await?,
		}

		Ok(id)
	}

	/// Whether `project` exists.
	pub async fn project_exists(&self, project: &ProjectId) -> Result<bool, StoreError> {
		match self {
			Self::Memory(store) => Ok(store.project_exists(project)),
			Self::Postgres(store) => store.project_exists(project).await,
		}
	}

	/// Store `key` for `project`'s `environment`.
	pub async fn add_api_key(
		&self,
		project: &ProjectId,
		environment: &Environment,
		key: &NewApiKey,
	) -> Result<(), StoreError> {
		match self {
			Self::Memory(store) => store.add_api_key(project, environment, key),
			Self::Postgres(store) => store.add_api_key(project, environment, key).await,
		}
	}

	/// The project environment an unrevoked API key with `hash` belongs to.
	pub async fn find_api_key(
		&self,
		hash: &KeyHash,
	) -> Result<Option<(ProjectId, Environment)>, StoreError> {
		match self {
			Self::Memory(store) => Ok(store.find_api_key(hash)),
			Self::Postgres(store) => store.find_api_key(hash).await,
		}
	}

	/// Set or clear the webhook for a project environment.
	pub async fn set_webhook(
		&self,
		project: &ProjectId,
		environment: &Environment,
		webhook: Option<&Webhook>,
	) -> Result<(), StoreError> {
		match self {
			Self::Memory(store) => {
				store.set_webhook(project, environment, webhook);
				Ok(())
			}
			Self::Postgres(store) => store.set_webhook(project, environment, webhook).await,
		}
	}

	/// The webhook for a project environment.
	pub async fn webhook(
		&self,
		project: &ProjectId,
		environment: &Environment,
	) -> Result<Option<Webhook>, StoreError> {
		match self {
			Self::Memory(store) => Ok(store.webhook(project, environment)),
			Self::Postgres(store) => store.webhook(project, environment).await,
		}
	}

	/// Count `key` as active in `month` unless its project's free plan is
	/// full. Documents already active this month are always admitted.
	pub async fn admit(&self, key: &DocumentKey, month: Month) -> Result<Admit, StoreError> {
		match self {
			Self::Memory(store) => store.admit(key, month),
			Self::Postgres(store) => store.admit(key, month).await,
		}
	}

	/// `project`'s billing terms.
	pub async fn terms(&self, project: &ProjectId) -> Result<Terms, StoreError> {
		match self {
			Self::Memory(store) => store.terms(project),
			Self::Postgres(store) => store.terms(project).await,
		}
	}

	/// Replace `project`'s billing terms.
	pub async fn set_terms(&self, project: &ProjectId, terms: &Terms) -> Result<(), StoreError> {
		match self {
			Self::Memory(store) => store.set_terms(project, terms),
			Self::Postgres(store) => store.set_terms(project, terms).await,
		}
	}

	/// `project`'s usage in `month`, per environment.
	pub async fn usage(
		&self,
		project: &ProjectId,
		month: Month,
	) -> Result<Vec<UsageRow>, StoreError> {
		match self {
			Self::Memory(store) => Ok(store.usage(project, month)),
			Self::Postgres(store) => store.usage(project, month).await,
		}
	}

	/// Count `count` conversions for a project environment in `month`.
	pub async fn add_conversions(
		&self,
		project: &ProjectId,
		environment: &Environment,
		month: Month,
		count: u64,
	) -> Result<(), StoreError> {
		match self {
			Self::Memory(store) => {
				store.add_conversions(project, environment, month, count);
				Ok(())
			}
			Self::Postgres(store) => {
				store
					.add_conversions(project, environment, month, count)
					.await
			}
		}
	}

	/// Everything stored for `key`.
	pub async fn load(&self, key: &DocumentKey) -> Result<StoredDocument, StoreError> {
		match self {
			Self::Memory(store) => Ok(store.load(key)),
			Self::Postgres(store) => store.load(key).await,
		}
	}

	/// Append a v1 update to `key` and return its sequence number.
	pub async fn append(&self, key: &DocumentKey, update: &[u8]) -> Result<i64, StoreError> {
		match self {
			Self::Memory(store) => Ok(store.append(key, update)),
			Self::Postgres(store) => store.append(key, update).await,
		}
	}

	/// Replace `key`'s snapshot with `snapshot`, which includes every update
	/// up to `through_seq`, and drop those updates.
	pub async fn compact(
		&self,
		key: &DocumentKey,
		snapshot: &[u8],
		through_seq: i64,
	) -> Result<(), StoreError> {
		match self {
			Self::Memory(store) => {
				store.compact(key, snapshot, through_seq);
				Ok(())
			}
			Self::Postgres(store) => store.compact(key, snapshot, through_seq).await,
		}
	}

	/// The documents in a project environment, by name.
	pub async fn list(
		&self,
		project: &ProjectId,
		environment: &Environment,
	) -> Result<Vec<DocumentSummary>, StoreError> {
		match self {
			Self::Memory(store) => Ok(store.list(project, environment)),
			Self::Postgres(store) => store.list(project, environment).await,
		}
	}

	/// Delete `key` and its history. Returns whether it existed.
	pub async fn delete(&self, key: &DocumentKey) -> Result<bool, StoreError> {
		match self {
			Self::Memory(store) => Ok(store.delete(key)),
			Self::Postgres(store) => store.delete(key).await,
		}
	}
}
