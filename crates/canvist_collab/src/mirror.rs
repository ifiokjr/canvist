//! The two-way translation between one model document and one `Y.Array` of
//! block maps.
//!
//! A [`Mirror`] does not own a yrs document: it works inside transactions
//! its owner opens. [`Binding`](crate::Binding) owns one over the root
//! `"blocks"` array; the scene binding owns one per text frame, over the
//! frame's nested `"text"` array, so every frame shares the scene's update
//! stream and undo history.

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
use yrs::DeepObservable;
use yrs::Origin;
use yrs::ReadTxn;
use yrs::TransactionMut;
use yrs::types::Events;

use crate::CollabError;
use crate::claims::Owners;
use crate::local::Local;
use crate::local::position_of;
use crate::registry::Registration;
use crate::registry::Registry;
use crate::registry::branch_id;
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
/// The key of the binding's own yrs observers.
pub(crate) const OBSERVER: &str = "canvist:binding";

/// Mirrors one document into one array of block maps and back.
pub(crate) struct Mirror {
	root: ArrayRef,
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
	/// A mirror over `root`, recording every change other peers (or undo)
	/// make inside it.
	pub(crate) fn new(root: ArrayRef, schema: Arc<Schema>) -> Self {
		let inbox = Arc::new(Mutex::new(Vec::new()));
		observe(&root, &schema, &inbox);

		Self {
			root,
			schema,
			registry: Registry::default(),
			inbox,
		}
	}

	/// The array this mirror reads and writes.
	pub(crate) fn root(&self) -> &ArrayRef {
		&self.root
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
	pub(crate) fn has_changes(&self) -> bool {
		!lock(&self.inbox).is_empty()
	}

	/// Write every block of `doc` into the empty array.
	pub(crate) fn seed(
		&mut self,
		txn: &mut TransactionMut<'_>,
		doc: &Document,
	) -> Result<(), CollabError> {
		if self.root.len(txn) > 0 {
			return Err(CollabError::NotEmpty);
		}

		let mut registration = Registration::default();

		// Inserting at the front never walks the array, so writing the blocks
		// last to first keeps seeding linear in the document's size.
		for &block in doc.children(None).iter().rev() {
			let tree = doc.tree(block).ok_or(CollabError::NotShared(block))?;
			tree::write(
				txn,
				&self.schema,
				(&self.root, 0, None),
				&tree,
				&mut registration,
			)?;
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
		client: u32,
	) -> Result<(Document, Vec<Repair>), CollabError> {
		lock(&self.inbox).clear();
		let mut doc = Document::empty(self.schema.clone(), client);
		let mut owners = Owners::default();
		owners.scan_array(&self.root, None, txn, &self.schema);
		let mut registration = Registration::default();
		let mut reader = Reader {
			txn,
			schema: &self.schema,
			claims: &mut owners,
		};
		let blocks = reader.children(&self.root, Parent::ROOT, &mut registration);

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
			root: &self.root,
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
		doc: &mut Document,
		retyped: &[BlockId],
	) -> Result<(Commit, Vec<Repair>), CollabError> {
		let remote = Remote::new(
			doc.transaction(),
			txn,
			(&self.root, &self.schema),
			&mut self.registry,
		);
		remote.reveal(retyped)
	}

	/// Translate the recorded remote changes into one commit on `doc`.
	pub(crate) fn mirror<T: ReadTxn>(
		&mut self,
		txn: &T,
		doc: &mut Document,
	) -> Result<(Commit, Vec<Repair>), CollabError> {
		let records = std::mem::take(&mut *lock(&self.inbox));
		let remote = Remote::new(
			doc.transaction(),
			txn,
			(&self.root, &self.schema),
			&mut self.registry,
		);
		remote.apply(records)
	}

	/// Delete block maps that lost their block id to another map.
	pub(crate) fn repair(&mut self, txn: &mut TransactionMut<'_>, repairs: &[Repair]) {
		for repair in repairs {
			let array = match repair.parent {
				None => Some(self.root.clone()),
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

impl Drop for Mirror {
	fn drop(&mut self) {
		self.root.unobserve_deep(OBSERVER);
	}
}

/// Subscribe the inbox to changes inside `root` that this peer did not make
/// from local commits.
fn observe(root: &ArrayRef, schema: &Arc<Schema>, inbox: &Arc<Mutex<Vec<Record>>>) {
	let schema = schema.clone();
	let inbox = inbox.clone();
	// A nested root array has an item id; records name it as "the root" so
	// the translation treats both kinds of root alike.
	let root_id = branch_id(root);
	root.observe_deep(
		OBSERVER,
		move |txn: &TransactionMut<'_>, events: &Events<'_>| {
			if is_local(txn) {
				return;
			}

			let records = events
				.iter()
				.filter_map(|event| record::record(txn, event, &schema, root_id));
			lock(&inbox).extend(records);
		},
	);
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
