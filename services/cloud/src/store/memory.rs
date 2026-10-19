//! The in-memory store, for tests and local runs without Postgres.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;

use super::DocumentSummary;
use super::StoreError;
use super::StoredDocument;
use super::Webhook;
use crate::auth::KeyHash;
use crate::auth::NewApiKey;
use crate::names::DocumentKey;
use crate::names::Environment;
use crate::names::ProjectId;

/// Everything in one mutex: the store is for tests, not throughput.
#[derive(Debug, Default)]
pub struct MemoryStore {
	inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
	projects: HashMap<ProjectId, String>,
	keys: HashMap<KeyHash, (ProjectId, Environment)>,
	webhooks: HashMap<(ProjectId, Environment), Webhook>,
	documents: BTreeMap<DocumentKey, MemoryDocument>,
	next_seq: i64,
}

#[derive(Debug, Default)]
struct MemoryDocument {
	snapshot: Option<(Vec<u8>, i64)>,
	updates: Vec<(i64, Vec<u8>)>,
}

impl MemoryStore {
	fn lock(&self) -> MutexGuard<'_, Inner> {
		// A panic while holding the lock leaves plain maps behind, which are
		// still consistent enough for a test store.
		self.inner
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
	}

	pub(super) fn create_project(&self, id: &ProjectId, name: &str) {
		self.lock().projects.insert(id.clone(), name.to_owned());
	}

	pub(super) fn project_exists(&self, id: &ProjectId) -> bool {
		self.lock().projects.contains_key(id)
	}

	pub(super) fn add_api_key(
		&self,
		project: &ProjectId,
		environment: &Environment,
		key: &NewApiKey,
	) -> Result<(), StoreError> {
		let mut inner = self.lock();

		if !inner.projects.contains_key(project) {
			return Err(StoreError::UnknownProject);
		}

		inner
			.keys
			.insert(key.hash, (project.clone(), environment.clone()));
		Ok(())
	}

	pub(super) fn find_api_key(&self, hash: &KeyHash) -> Option<(ProjectId, Environment)> {
		self.lock().keys.get(hash).cloned()
	}

	pub(super) fn set_webhook(
		&self,
		project: &ProjectId,
		environment: &Environment,
		webhook: Option<&Webhook>,
	) {
		let key = (project.clone(), environment.clone());
		let mut inner = self.lock();

		match webhook {
			Some(webhook) => inner.webhooks.insert(key, webhook.clone()),
			None => inner.webhooks.remove(&key),
		};
	}

	pub(super) fn webhook(
		&self,
		project: &ProjectId,
		environment: &Environment,
	) -> Option<Webhook> {
		self.lock()
			.webhooks
			.get(&(project.clone(), environment.clone()))
			.cloned()
	}

	pub(super) fn load(&self, key: &DocumentKey) -> StoredDocument {
		let inner = self.lock();
		let Some(document) = inner.documents.get(key) else {
			return StoredDocument::default();
		};
		let snapshot_seq = document.snapshot.as_ref().map_or(0, |(_, seq)| *seq);

		StoredDocument {
			snapshot: document.snapshot.as_ref().map(|(bytes, _)| bytes.clone()),
			updates: document
				.updates
				.iter()
				.map(|(_, update)| update.clone())
				.collect(),
			last_seq: document
				.updates
				.last()
				.map_or(snapshot_seq, |(seq, _)| *seq),
		}
	}

	pub(super) fn append(&self, key: &DocumentKey, update: &[u8]) -> i64 {
		let mut inner = self.lock();
		inner.next_seq += 1;
		let seq = inner.next_seq;
		inner
			.documents
			.entry(key.clone())
			.or_default()
			.updates
			.push((seq, update.to_vec()));
		seq
	}

	pub(super) fn compact(&self, key: &DocumentKey, snapshot: &[u8], through_seq: i64) {
		let mut inner = self.lock();
		let document = inner.documents.entry(key.clone()).or_default();
		document.snapshot = Some((snapshot.to_vec(), through_seq));
		document.updates.retain(|(seq, _)| *seq > through_seq);
	}

	pub(super) fn list(
		&self,
		project: &ProjectId,
		environment: &Environment,
	) -> Vec<DocumentSummary> {
		self.lock()
			.documents
			.iter()
			.filter(|(key, _)| &key.project == project && &key.environment == environment)
			.map(|(key, document)| {
				DocumentSummary {
					name: key.name.clone(),
					pending_updates: document.updates.len() as i64,
				}
			})
			.collect()
	}

	pub(super) fn delete(&self, key: &DocumentKey) -> bool {
		self.lock().documents.remove(key).is_some()
	}
}
