//! The in-memory store, for tests and local runs without Postgres.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;

use super::Admit;
use super::Appended;
use super::AppendedRows;
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
use crate::metering::Terms;
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
	terms: HashMap<ProjectId, Terms>,
	active: BTreeMap<(DocumentKey, Month), Activation>,
	usage: BTreeMap<(ProjectId, Month, Environment), Counters>,
}

/// A document's first activity in a month.
#[derive(Debug)]
struct Activation {
	/// The customer billed for it; `None` on unpaid plans.
	customer: Option<String>,
	first_active_at: i64,
	reported: bool,
}

#[derive(Debug, Default)]
struct Counters {
	active_documents: u64,
	conversions: u64,
	billable_conversions: u64,
	reported_conversions: u64,
	reserved_conversions: Option<u64>,
}

#[derive(Debug, Default)]
struct MemoryDocument {
	snapshot: Option<(Vec<u8>, i64)>,
	updates: Vec<(i64, Vec<u8>)>,
}

impl Inner {
	fn active_in(&self, project: &ProjectId, month: Month) -> u64 {
		self.usage
			.iter()
			.filter(|((owner, period, _), _)| owner == project && *period == month)
			.map(|(_, counters)| counters.active_documents)
			.sum()
	}
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
		let mut inner = self.lock();
		inner.projects.insert(id.clone(), name.to_owned());
		inner.terms.insert(id.clone(), Terms::of(Plan::Hobby));
	}

	pub(super) fn terms(&self, project: &ProjectId) -> Result<Terms, StoreError> {
		self.lock()
			.terms
			.get(project)
			.cloned()
			.ok_or(StoreError::UnknownProject)
	}

	pub(super) fn set_terms(&self, project: &ProjectId, terms: &Terms) -> Result<(), StoreError> {
		let mut inner = self.lock();
		let current = inner
			.terms
			.get_mut(project)
			.ok_or(StoreError::UnknownProject)?;
		*current = terms.clone();
		Ok(())
	}

	pub(super) fn admit(&self, key: &DocumentKey, month: Month) -> Result<Admit, StoreError> {
		let mut inner = self.lock();
		let terms = inner
			.terms
			.get(&key.project)
			.cloned()
			.ok_or(StoreError::UnknownProject)?;

		if inner.active.contains_key(&(key.clone(), month)) {
			return Ok(Admit {
				admitted: true,
				new: false,
				active: None,
				terms,
			});
		}

		let active = inner.active_in(&key.project, month);

		if terms.is_full(active) {
			return Ok(Admit {
				admitted: false,
				new: false,
				active: Some(active),
				terms,
			});
		}

		inner.active.insert(
			(key.clone(), month),
			Activation {
				customer: terms.metered_customer().map(str::to_owned),
				first_active_at: crate::auth::unix_now() as i64,
				reported: false,
			},
		);
		inner
			.usage
			.entry((key.project.clone(), month, key.environment.clone()))
			.or_default()
			.active_documents += 1;

		Ok(Admit {
			admitted: true,
			new: true,
			active: Some(active + 1),
			terms,
		})
	}

	pub(super) fn add_conversions(
		&self,
		project: &ProjectId,
		environment: &Environment,
		month: Month,
		count: u64,
	) {
		let mut inner = self.lock();
		let billable = inner
			.terms
			.get(project)
			.is_some_and(|terms| terms.metered_customer().is_some());
		let counters = inner
			.usage
			.entry((project.clone(), month, environment.clone()))
			.or_default();
		counters.conversions += count;

		if billable {
			counters.billable_conversions += count;
		}
	}

	pub(super) fn pending_documents(&self, limit: usize) -> Vec<PendingDocument> {
		let inner = self.lock();
		let mut pending: Vec<PendingDocument> = inner
			.active
			.iter()
			.filter(|(_, activation)| !activation.reported)
			.filter_map(|((key, month), activation)| {
				Some(PendingDocument {
					key: key.clone(),
					month: *month,
					customer: activation.customer.clone()?,
					first_active_at: activation.first_active_at,
				})
			})
			.collect();
		pending.sort_by_key(|document| document.first_active_at);
		pending.truncate(limit);
		pending
	}

	pub(super) fn mark_document_reported(&self, key: &DocumentKey, month: Month) {
		if let Some(activation) = self.lock().active.get_mut(&(key.clone(), month)) {
			activation.reported = true;
		}
	}

	pub(super) fn reserve_conversions(&self, limit: usize) -> Vec<PendingConversions> {
		let mut inner = self.lock();
		let Inner { usage, terms, .. } = &mut *inner;

		usage
			.iter_mut()
			.filter_map(|((project, month, environment), counters)| {
				let customer = terms.get(project)?.stripe_customer.clone()?;

				if counters.reserved_conversions.is_none()
					&& counters.billable_conversions > counters.reported_conversions
				{
					counters.reserved_conversions = Some(counters.billable_conversions);
				}

				Some(PendingConversions {
					project: project.clone(),
					environment: environment.clone(),
					month: *month,
					customer,
					from: counters.reported_conversions,
					to: counters.reserved_conversions?,
				})
			})
			.take(limit)
			.collect()
	}

	pub(super) fn mark_conversions_reported(&self, pending: &PendingConversions) {
		let key = (
			pending.project.clone(),
			pending.month,
			pending.environment.clone(),
		);

		if let Some(counters) = self.lock().usage.get_mut(&key)
			&& counters.reserved_conversions == Some(pending.to)
		{
			counters.reported_conversions = pending.to;
			counters.reserved_conversions = None;
		}
	}

	pub(super) fn usage(&self, project: &ProjectId, month: Month) -> Vec<UsageRow> {
		self.lock()
			.usage
			.iter()
			.filter(|((owner, period, _), _)| owner == project && *period == month)
			.map(|((_, _, environment), counters)| {
				UsageRow {
					environment: environment.clone(),
					active_documents: counters.active_documents,
					conversions: counters.conversions,
				}
			})
			.collect()
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

	pub(super) fn append_batch(&self, rows: &[(&DocumentKey, &[u8])]) -> Appended {
		let mut inner = self.lock();
		let mut appended = Appended::new();

		for (key, update) in rows {
			inner.next_seq += 1;
			let seq = inner.next_seq;
			inner
				.documents
				.entry((*key).clone())
				.or_default()
				.updates
				.push((seq, update.to_vec()));
			let entry = appended.entry((*key).clone()).or_insert(AppendedRows {
				rows: 0,
				last_seq: seq,
			});
			entry.rows += 1;
			entry.last_seq = seq;
		}

		appended
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
