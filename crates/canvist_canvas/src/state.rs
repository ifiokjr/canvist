//! The state tools work on, and the one path every change takes.

use canvist_layout::LayoutError;
use canvist_scene::Scene;
use canvist_scene::SceneCommit;
use canvist_scene::SceneError;
use canvist_scene::Step;
use canvist_scene::Transaction;
use kurbo::Size;

use crate::Camera;
use crate::SceneIndex;
use crate::Selection;
use crate::TextLayouts;
use crate::overlay::OverlayItem;

/// The viewport assumed until the host reports its size.
const DEFAULT_VIEWPORT: Size = Size::new(800.0, 600.0);

/// Why a canvas operation failed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CanvasError {
	/// A scene step was rejected.
	#[error(transparent)]
	Scene(#[from] SceneError),
	/// A text frame could not be laid out.
	#[error(transparent)]
	Layout(#[from] LayoutError),
	/// No registered extension provides this command.
	#[error("unknown command `{0}`")]
	UnknownCommand(String),
	/// No inspector field has this key.
	#[error("unknown inspector field `{0}`")]
	UnknownField(String),
	/// A command or field value did not match its schema.
	#[error("invalid arguments: {0}")]
	InvalidArgs(String),
}

/// A commit this canvas made, for a collaboration binding. `merge` says
/// whether it continues the previous undo step (a drag's later moves do).
#[derive(Clone, Debug, PartialEq)]
pub struct LocalCommit {
	/// The commit.
	pub commit: SceneCommit,
	/// Whether it joins the previous undo step.
	pub merge: bool,
}

/// Local undo history: each step is the commits of one gesture.
#[derive(Debug, Default)]
struct History {
	undo: Vec<Vec<SceneCommit>>,
	redo: Vec<Vec<SceneCommit>>,
}

/// Everything a tool reads and changes.
///
/// Every change goes through [`CanvasState::apply`] or
/// [`CanvasState::edit`], which apply steps to the scene, update the index
/// and text layouts, record history, and queue the commit for
/// collaboration. Commits from collaborators go through
/// [`CanvasState::apply_remote`].
#[derive(Debug)]
pub struct CanvasState {
	scene: Scene,
	index: SceneIndex,
	texts: TextLayouts,
	camera: Camera,
	selection: Selection,
	viewport: Size,
	history: History,
	outbox: Vec<LocalCommit>,
	/// Transient overlay items a tool set during its last input.
	pub(crate) chrome: Vec<OverlayItem>,
	/// Camera momentum after a pan.
	pub(crate) inertia: crate::Inertia,
}

impl CanvasState {
	/// State over `scene`, indexing it with `texts`.
	pub fn new(scene: Scene, mut texts: TextLayouts) -> Result<Self, CanvasError> {
		let index = SceneIndex::build(&scene, &mut texts)?;

		Ok(Self {
			scene,
			index,
			texts,
			camera: Camera::default(),
			selection: Selection::default(),
			viewport: DEFAULT_VIEWPORT,
			history: History::default(),
			outbox: Vec::new(),
			chrome: Vec::new(),
			inertia: crate::Inertia::default(),
		})
	}

	/// The scene.
	#[must_use]
	pub fn scene(&self) -> &Scene {
		&self.scene
	}

	/// The scene, for a collaboration binding that applies remote updates
	/// to it directly. Pass the commits it returns to
	/// [`CanvasState::apply_remote`].
	pub fn scene_mut(&mut self) -> &mut Scene {
		&mut self.scene
	}

	/// The derived geometry.
	#[must_use]
	pub fn index(&self) -> &SceneIndex {
		&self.index
	}

	/// Text frame layouts.
	#[must_use]
	pub fn texts(&self) -> &TextLayouts {
		&self.texts
	}

	/// The camera.
	#[must_use]
	pub fn camera(&self) -> &Camera {
		&self.camera
	}

	/// The camera, to pan and zoom.
	pub fn camera_mut(&mut self) -> &mut Camera {
		&mut self.camera
	}

	/// The selection.
	#[must_use]
	pub fn selection(&self) -> &Selection {
		&self.selection
	}

	/// The selection, to change it.
	pub fn selection_mut(&mut self) -> &mut Selection {
		&mut self.selection
	}

	/// The viewport size in screen pixels.
	#[must_use]
	pub fn viewport(&self) -> Size {
		self.viewport
	}

	/// Set the viewport size in screen pixels.
	pub fn set_viewport(&mut self, size: Size) {
		self.viewport = size;
	}

	/// Apply `steps` as one commit. `merge` joins the previous undo step.
	pub fn apply(&mut self, steps: Vec<Step>, merge: bool) -> Result<SceneCommit, CanvasError> {
		let commit = self.scene.apply_steps(steps)?;
		self.record(&commit, merge)?;
		Ok(commit)
	}

	/// Run `edit` in a scene transaction and record its commit.
	pub fn edit<T>(
		&mut self,
		merge: bool,
		edit: impl FnOnce(&mut Transaction<'_>) -> Result<T, SceneError>,
	) -> Result<(T, SceneCommit), CanvasError> {
		let mut transaction = self.scene.transaction();
		let value = edit(&mut transaction)?;
		let commit = transaction.commit();
		self.record(&commit, merge)?;
		Ok((value, commit))
	}

	/// Update derived state for a commit a collaborator made, which is
	/// already applied to the scene. Remote commits stay out of local undo.
	pub fn apply_remote(&mut self, commit: &SceneCommit) -> Result<(), CanvasError> {
		self.index.apply(&self.scene, commit, &mut self.texts)?;
		let scene = &self.scene;
		self.selection.retain(|id| scene.contains(id));
		Ok(())
	}

	/// Drain the commits made since the last call, for a collaboration
	/// binding.
	pub fn take_commits(&mut self) -> Vec<LocalCommit> {
		std::mem::take(&mut self.outbox)
	}

	/// Whether there is a local step to undo.
	#[must_use]
	pub fn can_undo(&self) -> bool {
		!self.history.undo.is_empty()
	}

	/// Whether there is an undone step to redo.
	#[must_use]
	pub fn can_redo(&self) -> bool {
		!self.history.redo.is_empty()
	}

	/// Undo the latest local step. Hosts that collaborate use the binding's
	/// undo instead, which knows about other peers' changes.
	pub fn undo(&mut self) -> Result<bool, CanvasError> {
		let Some(group) = self.history.undo.pop() else {
			return Ok(false);
		};
		let steps: Vec<Step> = group
			.iter()
			.rev()
			.flat_map(SceneCommit::undo_steps)
			.collect();
		let commit = self.scene.apply_steps(steps)?;
		self.after_history(&commit)?;
		self.history.redo.push(vec![commit]);
		Ok(true)
	}

	/// Redo the latest undone step.
	pub fn redo(&mut self) -> Result<bool, CanvasError> {
		let Some(group) = self.history.redo.pop() else {
			return Ok(false);
		};
		let steps: Vec<Step> = group
			.iter()
			.rev()
			.flat_map(SceneCommit::undo_steps)
			.collect();
		let commit = self.scene.apply_steps(steps)?;
		self.after_history(&commit)?;
		self.history.undo.push(vec![commit]);
		Ok(true)
	}

	/// Recompute the geometry of `nodes` (and what depends on them) from the
	/// scene, for changes made outside commits, such as a text frame's
	/// document coming back from an editor.
	pub fn refresh_nodes(&mut self, nodes: &[canvist_scene::NodeId]) -> Result<(), CanvasError> {
		self.index.refresh(&self.scene, nodes, &mut self.texts)?;
		Ok(())
	}

	/// Recompute text layout and geometry for a lent text frame whose
	/// document an editor changed.
	pub fn refresh_text(
		&mut self,
		node: canvist_scene::NodeId,
		doc: &canvist_model::Document,
	) -> Result<(), CanvasError> {
		self.texts.update(&self.scene, node, doc)?;
		self.index.refresh(&self.scene, &[node], &mut self.texts)?;
		Ok(())
	}

	/// Record a commit made outside the canvas's own paths (a text session's
	/// summary) as one undo step.
	pub fn record_external(&mut self, commit: &SceneCommit) -> Result<(), CanvasError> {
		self.record(commit, false)
	}

	fn record(&mut self, commit: &SceneCommit, merge: bool) -> Result<(), CanvasError> {
		if commit.is_empty() {
			return Ok(());
		}

		self.index.apply(&self.scene, commit, &mut self.texts)?;
		let scene = &self.scene;
		self.selection.retain(|id| scene.contains(id));

		match self.history.undo.last_mut() {
			Some(group) if merge => group.push(commit.clone()),
			_ => self.history.undo.push(vec![commit.clone()]),
		}

		self.history.redo.clear();
		self.outbox.push(LocalCommit {
			commit: commit.clone(),
			merge,
		});
		Ok(())
	}

	fn after_history(&mut self, commit: &SceneCommit) -> Result<(), CanvasError> {
		self.index.apply(&self.scene, commit, &mut self.texts)?;
		let scene = &self.scene;
		self.selection.retain(|id| scene.contains(id));
		self.outbox.push(LocalCommit {
			commit: commit.clone(),
			merge: false,
		});
		Ok(())
	}
}
