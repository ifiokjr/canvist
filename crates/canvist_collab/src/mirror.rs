//! The two-way translation between one model document and one `Y.Array` of
//! block maps.
//!
//! A [`Mirror`] owns neither a yrs document nor a reference into one: it
//! works inside transactions its owner opens, on the array its owner passes
//! in. [`Binding`](crate::Binding) passes the root `"blocks"` array; the
//! scene binding passes each text frame's nested `"text"` array, looked up
//! afresh in every transaction, because yrs frees a deleted nested type and
//! a reference held across transactions could outlive it. The owner's
//! observer feeds the mirror's inbox through [`Mirror::record`].

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use canvist_model::BlockId;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::Schema;
use canvist_model::Step;
use yrs::Array;
use yrs::ArrayRef;
use yrs::Origin;
use yrs::ReadTxn;
use yrs::TransactionMut;

use crate::CollabError;
use crate::claims::Owners;
use crate::local::Local;
use crate::local::position_of;
use crate::registry::Registration;
use crate::registry::Registry;
use crate::remote::Remote;
use crate::remote::Repair;
use crate::remote::record;
use crate::remote::record::Record;
use crate::tree;
use crate::tree::Parent;
use crate::tree::Reader;

/// Transactions created from local commits with undo tracking.
pub(crate) const LOCAL: &str = "canvist:local";
/// Local transactions undo ignores: untracked edits and duplicate repairs.
pub(crate) const UNTRACKED: &str = "canvist:untracked";
/// Transactions that integrate updates from other peers.
pub(crate) const REMOTE: &str = "canvist:remote";
/// The key of the bindings' own yrs observers.
pub(crate) const OBSERVER: &str = "canvist:binding";

/// Mirrors one document into one array of block maps and back.
pub(crate) struct Mirror {
	schema: Arc<Schema>,
	registry: Registry,
	inbox: Arc<Mutex<Vec<Record>>>,
}

impl std::fmt::Debug for Mirror {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Mirror")
			.field("registry", &self.registry)
			.finish_non_exhaustive()
	}
}

impl Mirror {
	/// An empty mirror. Its owner records remote changes into it with
	/// [`Mirror::record`].
	pub(crate) fn new(schema: Arc<Schema>) -> Self {
		Self {
			schema,
			registry: Registry::default(),
			inbox: Arc::new(Mutex::new(Vec::new())),
		}
	}

	/// Queue records an owner's observer took from a nested array.
	#[cfg(feature = "scene")]
	pub(crate) fn receive(&self, records: Vec<Record>) {
		lock(&self.inbox).extend(records);
	}

	/// The inbox, for an observer that outlives a borrow of the mirror.
	pub(crate) fn inbox(&self) -> Arc<Mutex<Vec<Record>>> {
		self.inbox.clone()
	}

	/// The schema of the mirrored document.
	pub(crate) fn schema(&self) -> &Arc<Schema> {
		&self.schema
	}

	/// Which shared types mirror which blocks.
	pub(crate) fn registry(&self) -> &Registry {
		&self.registry
	}

	/// How many block maps the model hides because they break the schema.
	pub(crate) fn rejected_blocks(&self) -> u32 {
		self.registry.unmirrored_total()
	}

	/// Whether remote changes are waiting to be mirrored.
	#[cfg(feature = "scene")]
	pub(crate) fn has_changes(&self) -> bool {
		!lock(&self.inbox).is_empty()
	}

	/// Write every block of `doc` into the empty array.
	pub(crate) fn seed(
		&mut self,
		txn: &mut TransactionMut<'_>,
		root: &ArrayRef,
		doc: &Document,
	) -> Result<(), CollabError> {
		if root.len(txn) > 0 {
			return Err(CollabError::NotEmpty);
		}

		let mut registration = Registration::default();

		// Inserting at the front never walks the array, so writing the blocks
		// last to first keeps seeding linear in the document's size.
		for &block in doc.children(None).iter().rev() {
			let tree = doc.tree(block).ok_or(CollabError::NotShared(block))?;
			tree::write(txn, &self.schema, (root, 0, None), &tree, &mut registration)?;
		}

		self.registry = Registry::default();
		self.registry.register(registration);
		Ok(())
	}

	/// Build a document from the array. Returns it with the duplicate block
	/// maps the owner must delete through [`Mirror::repair`].
	pub(crate) fn document<T: ReadTxn>(
		&mut self,
		txn: &T,
		root: &ArrayRef,
		client: u32,
	) -> Result<(Document, Vec<Repair>), CollabError> {
		lock(&self.inbox).clear();
		let mut doc = Document::empty(self.schema.clone(), client);
		let mut owners = Owners::default();
		owners.scan_array(root, None, txn, &self.schema);
		let mut registration = Registration::default();
		let mut reader = Reader {
			txn,
			schema: &self.schema,
			claims: &mut owners,
		};
		let blocks = reader.children(root, Parent::ROOT, &mut registration);

		doc.apply_steps([Step::InsertBlocks {
			parent: None,
			index: 0,
			blocks,
		}])?;
		let repairs = repairs_of(&registration);
		self.registry = Registry::default();
		self.registry.register(registration);
		Ok((doc, repairs))
	}

	/// Translate a local commit of the mirrored document, which `doc` must be
	/// in its state right after `commit`. Returns the containers it retyped
	/// while they held hidden children.
	pub(crate) fn translate(
		&mut self,
		txn: &mut TransactionMut<'_>,
		root: &ArrayRef,
		doc: &Document,
		commit: &Commit,
	) -> Result<Vec<BlockId>, CollabError> {
		let structural = commit
			.steps
			.iter()
			.filter(|step| is_structural(step))
			.count();
		let mut local = Local {
			txn,
			root,
			schema: &self.schema,
			registry: &mut self.registry,
			doc,
			single_structural: structural <= 1,
			settled: crate::local::settled(&commit.steps),
			retyped: Vec::new(),
		};

		for (step, inverse) in commit.steps.iter().zip(&commit.inverses) {
			local.step(step, inverse)?;
		}

		Ok(local.retyped)
	}

	/// Show hidden children that the new types of `retyped` allow.
	pub(crate) fn reveal<T: ReadTxn>(
		&mut self,
		txn: &T,
		root: &ArrayRef,
		doc: &mut Document,
		retyped: &[BlockId],
	) -> Result<(Commit, Vec<Repair>), CollabError> {
		let remote = Remote::new(
			doc.transaction(),
			txn,
			(root, &self.schema),
			&mut self.registry,
		);
		remote.reveal(retyped)
	}

	/// Translate the recorded remote changes into one commit on `doc`.
	pub(crate) fn mirror<T: ReadTxn>(
		&mut self,
		txn: &T,
		root: &ArrayRef,
		doc: &mut Document,
	) -> Result<(Commit, Vec<Repair>), CollabError> {
		let records = std::mem::take(&mut *lock(&self.inbox));
		let remote = Remote::new(
			doc.transaction(),
			txn,
			(root, &self.schema),
			&mut self.registry,
		);
		remote.apply(records)
	}

	/// Delete block maps that lost their block id to another map.
	pub(crate) fn repair(
		&mut self,
		txn: &mut TransactionMut<'_>,
		root: &ArrayRef,
		repairs: &[Repair],
	) {
		for repair in repairs {
			let array = match repair.parent {
				None => Some(root.clone()),
				Some(parent) => {
					self.registry
						.block(parent)
						.and_then(|found| crate::local::map_ref(txn, &found.map))
						.and_then(|map| tree::children_of(&map, txn))
				}
			};

			if let Some(array) = array
				&& let Some(index) = position_of(&array, txn, &repair.map)
			{
				array.remove(txn, index);
			}

			self.registry.remove_hidden(&repair.map);
		}
	}
}

/// Record every event inside a mirrored array that this peer did not make
/// from local commits. `root` is the mirrored array's item id when it is
/// nested inside another shared type, or `None` for a root type.
pub(crate) fn record_events<'e>(
	txn: &TransactionMut<'_>,
	events: impl IntoIterator<Item = &'e yrs::types::Event>,
	schema: &Schema,
	root: Option<yrs::ID>,
	inbox: &Mutex<Vec<Record>>,
) {
	let records = events
		.into_iter()
		.filter_map(|event| record::record(txn, event, schema, root));
	lock(inbox).extend(records);
}

/// Whether `txn` was made by this peer from local commits.
pub(crate) fn is_local(txn: &TransactionMut<'_>) -> bool {
	txn.origin()
		.is_some_and(|origin| *origin == Origin::from(LOCAL) || *origin == Origin::from(UNTRACKED))
}

fn is_structural(step: &Step) -> bool {
	matches!(
		step,
		Step::Split { .. }
			| Step::Join { .. }
			| Step::InsertBlocks { .. }
			| Step::RemoveBlocks { .. }
			| Step::MoveBlocks { .. }
	)
}

fn repairs_of(registration: &Registration) -> Vec<Repair> {
	registration
		.losers
		.iter()
		.map(|&(map, parent)| Repair { parent, map })
		.collect()
}

/// Lock a mutex shared with a yrs observer. A panic inside an observer
/// leaves the data intact (it is only ever appended to), so poisoning is
/// ignored.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
	mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
