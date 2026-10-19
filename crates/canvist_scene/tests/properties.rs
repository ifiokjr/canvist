//! Property tests: random edit sequences keep every invariant, failed steps
//! change nothing, undoing every commit restores the original exactly,
//! redoing reaches the same final scene, and JSON round-trips.

mod support;

use std::sync::Arc;

use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_scene::Color;
use canvist_scene::NodeId;
use canvist_scene::Paint;
use canvist_scene::Prop;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::SceneCommit;
use canvist_scene::SceneError;
use canvist_scene::Shape;
use kurbo::Affine;
use kurbo::Vec2;
use proptest::prelude::*;
use serde_json::json;

const KINDS: [&str; 8] = [
	"page",
	"frame",
	"group",
	"rectangle",
	"ellipse",
	"polygon",
	"path",
	"text",
];

/// An edit described by seeds, resolved against the current scene.
#[derive(Clone, Debug)]
enum Edit {
	Insert {
		parent: usize,
		index: usize,
		kind: usize,
	},
	Remove {
		node: usize,
	},
	Move {
		node: usize,
		parent: usize,
		index: usize,
	},
	Set {
		node: usize,
		prop: usize,
		value: u8,
	},
	Type {
		node: usize,
		text: String,
	},
	Batch {
		edits: Vec<(usize, usize)>,
	},
}

fn edit() -> impl Strategy<Value = Edit> {
	let seed = || 0..64usize;
	prop_oneof![
		4 => (seed(), seed(), 0..KINDS.len()).prop_map(|(parent, index, kind)| Edit::Insert { parent, index, kind }),
		1 => seed().prop_map(|node| Edit::Remove { node }),
		3 => (seed(), seed(), seed()).prop_map(|(node, parent, index)| Edit::Move { node, parent, index }),
		4 => (seed(), 0..8usize, any::<u8>()).prop_map(|(node, prop, value)| Edit::Set { node, prop, value }),
		2 => (seed(), "[a-z🎨 ]{1,5}").prop_map(|(node, text)| Edit::Type { node, text }),
		1 => prop::collection::vec((seed(), seed()), 1..4).prop_map(|edits| Edit::Batch { edits }),
	]
}

/// The root and every node in paint order, so seeds pick deterministically.
fn nodes(scene: &Scene) -> Vec<NodeId> {
	std::iter::once(NodeId::ROOT)
		.chain(scene.subtree(NodeId::ROOT))
		.collect()
}

fn pick(scene: &Scene, seed: usize) -> NodeId {
	let all = nodes(scene);
	all[seed % all.len()]
}

fn prop_for(scene: &Scene, node: NodeId, which: usize, value: u8) -> Prop {
	let amount = f64::from(value);

	match which {
		0 => Prop::Opacity(amount / 255.0),
		1 => {
			Prop::Transform(
				Affine::rotate(amount / 40.0).then_translate(Vec2::new(amount, -amount)),
			)
		}
		2 => Prop::Size(Vec2::new(amount, amount / 2.0)),
		3 => Prop::Fills(Arc::new([Paint::solid(Color::rgb(value, 255 - value, 7))])),
		4 => Prop::Name(format!("n{value}").into()),
		5 => {
			Prop::Custom(
				"seed".into(),
				(!value.is_multiple_of(3)).then(|| json!(value)),
			)
		}
		6 => {
			match scene.kind(node) {
				Some(canvist_scene::NodeKind::Polygon) => {
					Prop::Shape(Shape::polygon(3 + u32::from(value % 9)))
				}
				_ => Prop::Shape(Shape::ellipse()),
			}
		}
		_ => Prop::Visible(value.is_multiple_of(2)),
	}
}

/// Apply one edit in its own transaction. Returns the commit, or `None` when
/// the edit was rejected, in which case the scene must be unchanged.
fn apply(scene: &mut Scene, edit: &Edit) -> Result<Option<SceneCommit>, TestCaseError> {
	let before = scene.to_json();
	let mut transaction = scene.transaction();
	if run(&mut transaction, edit).is_ok() {
		return Ok(Some(transaction.commit()));
	}

	drop(transaction);
	prop_assert_eq!(scene.to_json(), before, "a rejected edit changed the scene");
	Ok(None)
}

fn run(transaction: &mut canvist_scene::Transaction<'_>, edit: &Edit) -> Result<(), SceneError> {
	match edit {
		Edit::Insert {
			parent,
			index,
			kind,
		} => {
			let parent = pick(transaction.scene(), *parent);
			transaction.insert(parent, Some(*index % 4), KINDS[*kind], Props::default())?;
		}
		Edit::Remove { node } => {
			let node = pick(transaction.scene(), *node);
			transaction.remove(&[node])?;
		}
		Edit::Move {
			node,
			parent,
			index,
		} => {
			let node = pick(transaction.scene(), *node);
			let parent = pick(transaction.scene(), *parent);
			transaction.move_to(node, parent, Some(*index % 4))?;
		}
		Edit::Set { node, prop, value } => {
			let node = pick(transaction.scene(), *node);
			let prop = prop_for(transaction.scene(), node, *prop, *value);
			transaction.set(node, prop)?;
		}
		Edit::Type { node, text } => {
			let node = pick(transaction.scene(), *node);
			transaction.text(node, |inner| {
				let block = inner
					.doc()
					.first_text_block()
					.expect("text frames keep a paragraph");
				inner.insert_text(Point::new(block, 0), text, MarkSet::empty())
			})?;
		}
		Edit::Batch { edits } => {
			// A multi-step transaction: a failure part-way rolls back the whole
			// edit, which `apply` checks.
			for &(node, parent) in edits {
				let node = pick(transaction.scene(), node);
				let parent = pick(transaction.scene(), parent);
				transaction.insert(parent, None, "rectangle", Props::default())?;
				transaction.set(node, Prop::Locked(true))?;
			}
		}
	}

	Ok(())
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(256))]

	#[test]
	fn random_edits_keep_invariants_and_undo_exactly(edits in prop::collection::vec(edit(), 1..40)) {
		let mut scene = Scene::new(support::schema(), 1).expect("new scene");
		let original = scene.to_json();
		let mut commits = Vec::new();

		for edit in &edits {
			if let Some(commit) = apply(&mut scene, edit)? {
				commits.push(commit);
			}

			prop_assert_eq!(scene.check(), Ok(()), "after {:?}", edit);
		}

		let last = scene.to_json();
		let reloaded = Scene::from_json(support::schema(), 2, &last).expect("own JSON loads");
		prop_assert_eq!(reloaded.to_json(), last.clone());

		for commit in commits.iter().rev() {
			scene.apply_steps(commit.undo_steps()).expect("undo applies");
			prop_assert_eq!(scene.check(), Ok(()));
		}

		prop_assert_eq!(scene.to_json(), original);

		for commit in &commits {
			scene.apply_steps(commit.steps.clone()).expect("redo applies");
		}

		prop_assert_eq!(scene.to_json(), last);
	}
}
