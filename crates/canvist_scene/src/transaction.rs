//! Transactions group steps into one atomic, undoable change.

use canvist_model::StepError;
use rustc_hash::FxHashSet;

use crate::NodeId;
use crate::NodeRecord;
use crate::Prop;
use crate::Props;
use crate::Scene;
use crate::SceneError;
use crate::Step;

/// Applies steps to a scene and records what changed.
///
/// Steps apply eagerly, so later steps see earlier ones. Call
/// [`Transaction::commit`] to keep the changes; dropping the transaction
/// without committing rolls them back.
#[derive(Debug)]
pub struct Transaction<'scene> {
	scene: &'scene mut Scene,
	steps: Vec<Step>,
	inverses: Vec<Step>,
	touched: FxHashSet<NodeId>,
	structural: bool,
	finished: bool,
}

impl<'scene> Transaction<'scene> {
	/// Start a transaction on `scene`.
	pub fn new(scene: &'scene mut Scene) -> Self {
		Self {
			scene,
			steps: Vec::new(),
			inverses: Vec::new(),
			touched: FxHashSet::default(),
			structural: false,
			finished: false,
		}
	}

	/// The scene as changed so far.
	#[must_use]
	pub fn scene(&self) -> &Scene {
		self.scene
	}

	/// Whether no steps have been applied.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.steps.is_empty()
	}

	/// Apply one step. On error nothing changes and earlier steps remain.
	pub fn step(&mut self, step: Step) -> Result<(), SceneError> {
		let applied = step.clone().apply(self.scene)?;
		self.steps.push(step);
		self.inverses.push(applied.inverse);
		self.touched.extend(applied.touched);
		self.structural |= applied.structural;
		Ok(())
	}

	/// Insert a node of the type named `kind` under `parent` at `index`
	/// (`None` for the end, on top), returning its id. Props that leave the
	/// shape at [`Shape::None`](crate::Shape::None) get the type's default
	/// shape.
	pub fn insert(
		&mut self,
		parent: NodeId,
		index: Option<usize>,
		kind: &str,
		props: Props,
	) -> Result<NodeId, SceneError> {
		let schema = self.scene.schema().clone();
		let kind = schema
			.node_type(kind)
			.ok_or_else(|| SceneError::UnknownType(kind.to_owned()))?;
		let mut props = props;

		if props.shape == crate::Shape::None {
			props.shape = crate::Shape::default_for(schema.spec(kind).kind());
		}

		let id = self.scene.allocate_id()?;
		let key = self.scene.key_at(parent, index, None)?;
		let record = NodeRecord::new(id, kind, parent, key).with_props(props);
		self.step(Step::InsertNodes {
			nodes: vec![record],
		})?;
		Ok(id)
	}

	/// Remove nodes and their descendants.
	pub fn remove(&mut self, ids: &[NodeId]) -> Result<(), SceneError> {
		self.step(Step::RemoveNodes { ids: ids.to_vec() })
	}

	/// Move `node` under `parent` at `index` among the other children
	/// (`None` for the end).
	pub fn move_to(
		&mut self,
		node: NodeId,
		parent: NodeId,
		index: Option<usize>,
	) -> Result<(), SceneError> {
		let key = self.scene.key_at(parent, index, Some(node))?;
		self.step(Step::Move { node, parent, key })
	}

	/// Set one property.
	pub fn set(&mut self, node: NodeId, prop: Prop) -> Result<(), SceneError> {
		self.step(Step::SetProp { node, prop })
	}

	/// Edit a text frame's document with a text-model transaction. Each of
	/// its steps becomes a [`Step::Text`]; if `edit` fails, its steps are
	/// rolled back and the error is returned.
	pub fn text<T>(
		&mut self,
		node: NodeId,
		edit: impl FnOnce(&mut canvist_model::Transaction<'_>) -> Result<T, StepError>,
	) -> Result<T, SceneError> {
		let mut doc = self.scene.lend_text(node)?;
		let mut transaction = doc.transaction();
		let outcome = match edit(&mut transaction) {
			Ok(value) => Ok((value, transaction.commit())),
			Err(source) => Err(transaction.rollback().err().unwrap_or(source)),
		};
		self.scene.return_text(node, doc)?;
		let (value, commit) = outcome.map_err(|source| SceneError::Text { node, source })?;
		self.record_text(node, commit);
		Ok(value)
	}

	/// Let `change` edit a text frame's document directly. It must return the
	/// commit of exactly the steps it applied, and leave the document
	/// unchanged when it fails. Each step becomes a [`Step::Text`]. A
	/// collaboration binding uses this to integrate a peer's text changes,
	/// as `Editor::apply_external` does for the text editor.
	pub fn text_external<E: From<SceneError>>(
		&mut self,
		node: NodeId,
		change: impl FnOnce(&mut canvist_model::Document) -> Result<canvist_model::Commit, E>,
	) -> Result<(), E> {
		let mut doc = self.scene.lend_text(node)?;
		let outcome = change(&mut doc);
		self.scene.return_text(node, doc)?;
		self.record_text(node, outcome?);
		Ok(())
	}

	fn record_text(&mut self, node: NodeId, commit: canvist_model::Commit) {
		if commit.is_empty() {
			return;
		}

		for (step, inverse) in commit.steps.into_iter().zip(commit.inverses) {
			self.steps.push(Step::Text { node, step });
			self.inverses.push(Step::Text {
				node,
				step: inverse,
			});
		}

		self.touched.insert(node);
	}

	/// Undo every step and release the scene.
	pub fn rollback(mut self) -> Result<(), SceneError> {
		self.finished = true;
		undo_all(self.scene, std::mem::take(&mut self.inverses))
	}

	/// Finish the transaction and describe what it did.
	#[must_use]
	pub fn commit(mut self) -> SceneCommit {
		self.finished = true;
		let mut touched: Vec<NodeId> = std::mem::take(&mut self.touched)
			.into_iter()
			.filter(|&id| self.scene.contains(id))
			.collect();
		touched.sort_unstable();

		SceneCommit {
			steps: std::mem::take(&mut self.steps),
			inverses: std::mem::take(&mut self.inverses),
			touched,
			structural: self.structural,
			version: self.scene.version(),
		}
	}
}

impl Drop for Transaction<'_> {
	fn drop(&mut self) {
		if self.finished {
			return;
		}

		// Inverses of applied steps always apply unless the scene was changed
		// behind the transaction's back, which `&mut` borrowing prevents.
		let _ = undo_all(self.scene, std::mem::take(&mut self.inverses));
	}
}

fn undo_all(scene: &mut Scene, inverses: Vec<Step>) -> Result<(), SceneError> {
	for inverse in inverses.into_iter().rev() {
		inverse.apply(scene)?;
	}

	Ok(())
}

/// A committed transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCommit {
	/// The steps, in application order. Collaboration bindings replay these.
	pub steps: Vec<Step>,
	/// The inverse of each step, in application order.
	pub inverses: Vec<Step>,
	/// Surviving nodes whose props, position, or text changed, and parents
	/// whose child lists changed, sorted by id.
	pub touched: Vec<NodeId>,
	/// Whether any child list changed.
	pub structural: bool,
	/// The scene version after the commit.
	pub version: u64,
}

impl SceneCommit {
	/// The steps that undo this commit, in the order to apply them.
	#[must_use]
	pub fn undo_steps(&self) -> Vec<Step> {
		self.inverses.iter().rev().cloned().collect()
	}

	/// Whether the commit changed nothing.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.steps.is_empty()
	}
}

impl Scene {
	/// Start a [`Transaction`].
	pub fn transaction(&mut self) -> Transaction<'_> {
		Transaction::new(self)
	}

	/// Apply steps produced elsewhere (undo, redo, or a peer) as one commit.
	/// On error the steps applied so far are rolled back.
	pub fn apply_steps(
		&mut self,
		steps: impl IntoIterator<Item = Step>,
	) -> Result<SceneCommit, SceneError> {
		let mut transaction = self.transaction();

		for step in steps {
			transaction.step(step)?;
		}

		Ok(transaction.commit())
	}
}
