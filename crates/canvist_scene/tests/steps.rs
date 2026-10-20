//! Scenario tests for each step kind, its errors, and scene JSON.

mod support;

use std::sync::Arc;

use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_scene::Color;
use canvist_scene::FracKey;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::NodeSpec;
use canvist_scene::Paint;
use canvist_scene::Prop;
use canvist_scene::PropKey;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::SceneError;
use canvist_scene::SceneSchema;
use canvist_scene::Shape;
use canvist_scene::Step;
use kurbo::BezPath;
use kurbo::Vec2;
use serde_json::json;

fn names(scene: &Scene, parent: NodeId) -> Vec<String> {
	scene
		.children(parent)
		.iter()
		.map(|&id| {
			scene
				.node(id)
				.expect("child exists")
				.props()
				.name
				.to_string()
		})
		.collect()
}

fn named(name: &str) -> Props {
	Props {
		name: name.into(),
		..Props::default()
	}
}

#[test]
fn inserts_in_order_and_undoes_exactly() -> Result<(), SceneError> {
	let mut scene = Scene::new(support::schema(), 1)?;
	let page = scene.pages()[0];
	let before = scene.to_json();

	let mut transaction = scene.transaction();
	let a = transaction.insert(page, None, "rectangle", named("a"))?;
	transaction.insert(page, None, "ellipse", named("c"))?;
	transaction.insert(page, Some(1), "polygon", named("b"))?;
	transaction.insert(page, Some(0), "path", named("first"))?;
	let commit = transaction.commit();

	assert_eq!(names(&scene, page), ["first", "a", "b", "c"]);
	assert_eq!(
		scene.node(a).map(|node| node.props().shape.clone()),
		Some(Shape::Rect)
	);
	assert!(commit.structural);
	assert!(commit.touched.contains(&page));
	assert_eq!(scene.check(), Ok(()));

	scene.apply_steps(commit.undo_steps())?;
	assert_eq!(scene.to_json(), before);

	scene.apply_steps(commit.steps.clone())?;
	assert_eq!(names(&scene, page), ["first", "a", "b", "c"]);
	Ok(())
}

#[test]
fn removing_restores_subtrees_and_text() -> Result<(), SceneError> {
	let mut scene = Scene::new(support::schema(), 1)?;
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let frame = transaction.insert(page, None, "frame", named("card"))?;
	let group = transaction.insert(frame, None, "group", named("group"))?;
	transaction.insert(group, None, "rectangle", named("inner"))?;
	let text = transaction.insert(frame, None, "text", named("title"))?;
	transaction.text(text, |text| {
		let block = text.doc().first_text_block().expect("one paragraph");
		text.insert_text(Point::new(block, 0), "Wet paint", MarkSet::empty())
	})?;
	let _ = transaction.commit();
	let before = scene.to_json();

	let mut transaction = scene.transaction();
	transaction.remove(&[group, frame, group])?;
	let commit = transaction.commit();

	assert_eq!(scene.len(), 1);
	assert!(scene.text(text).is_none());

	scene.apply_steps(commit.undo_steps())?;
	assert_eq!(scene.to_json(), before);
	assert_eq!(
		scene
			.text(text)
			.map(canvist_model::Document::plain_text)
			.as_deref(),
		Some("Wet paint")
	);
	Ok(())
}

#[test]
fn moves_reparent_and_reject_cycles() -> Result<(), SceneError> {
	let mut scene = Scene::new(support::schema(), 1)?;
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let outer = transaction.insert(page, None, "frame", named("outer"))?;
	let inner = transaction.insert(outer, None, "group", named("inner"))?;
	let shape = transaction.insert(page, None, "rectangle", named("shape"))?;
	let leaf = transaction.insert(page, None, "ellipse", named("leaf"))?;
	let _ = transaction.commit();

	let mut transaction = scene.transaction();
	transaction.move_to(shape, inner, None)?;
	transaction.move_to(outer, page, Some(0))?;
	let commit = transaction.commit();
	assert_eq!(
		scene.node(shape).map(canvist_scene::Node::parent),
		Some(inner)
	);

	let before = scene.to_json();
	let mut transaction = scene.transaction();
	let cycle = transaction.move_to(outer, inner, None);
	assert_eq!(cycle, Err(SceneError::Cycle(outer)));
	let into_leaf = transaction.move_to(inner, leaf, None);
	assert!(matches!(into_leaf, Err(SceneError::NotAccepted { .. })));
	let to_root = transaction.move_to(shape, NodeId::ROOT, None);
	assert!(matches!(to_root, Err(SceneError::NotAccepted { .. })));
	drop(transaction);
	assert_eq!(scene.to_json(), before);

	scene.apply_steps(commit.undo_steps())?;
	assert_eq!(
		scene.node(shape).map(canvist_scene::Node::parent),
		Some(page)
	);
	Ok(())
}

#[test]
fn props_validate_against_the_node_type() -> Result<(), SceneError> {
	let mut scene = Scene::new(support::schema(), 1)?;
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let rect = transaction.insert(page, None, "rectangle", Props::default())?;

	let bad = [
		Prop::Opacity(1.5),
		Prop::Size(Vec2::new(-1.0, 4.0)),
		Prop::Shape(Shape::ellipse()),
		Prop::Shape(Shape::Polygon {
			points: 2,
			ratio: 1.0,
		}),
		Prop::Override(rect, Box::new(Prop::Visible(false))),
	];

	for prop in bad {
		assert!(
			matches!(
				transaction.set(rect, prop.clone()),
				Err(SceneError::InvalidProp { .. })
			),
			"{prop:?} should be rejected"
		);
	}

	transaction.set(rect, Prop::Fills(Arc::new([Paint::solid(Color::WHITE)])))?;
	transaction.set(rect, Prop::Custom("seed".into(), Some(json!(42))))?;
	let commit = transaction.commit();
	let node = scene.node(rect).expect("inserted");

	assert_eq!(node.props().custom.get("seed"), Some(&json!(42)));
	assert_eq!(commit.steps.len(), 3);
	Ok(())
}

#[test]
fn instances_carry_overrides() -> Result<(), SceneError> {
	let mut scene = Scene::new(support::schema(), 1)?;
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let component = transaction.insert(page, None, "component", named("button"))?;
	let label = transaction.insert(component, None, "rectangle", named("label"))?;
	let instance = transaction.insert(
		page,
		None,
		"instance",
		Props {
			shape: Shape::Instance { component },
			..Props::default()
		},
	)?;
	transaction.set(
		instance,
		Prop::Override(label, Box::new(Prop::Opacity(0.5))),
	)?;
	let commit = transaction.commit();
	let props = scene.node(instance).expect("inserted").props();

	assert_eq!(
		props.get(&PropKey::Override(label, Box::new(PropKey::Opacity))),
		Prop::Override(label, Box::new(Prop::Opacity(0.5)))
	);

	let json = scene.to_json();
	let reloaded = Scene::from_json(support::schema(), 2, &json)?;
	assert_eq!(reloaded.to_json(), json);
	scene.apply_steps(commit.undo_steps())?;
	assert_eq!(scene.len(), 1);
	Ok(())
}

#[test]
fn text_steps_fail_on_lent_and_non_text_nodes() -> Result<(), SceneError> {
	let mut scene = Scene::new(support::schema(), 1)?;
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let text = transaction.insert(page, None, "text", Props::default())?;
	let rect = transaction.insert(page, None, "rectangle", Props::default())?;
	let _ = transaction.commit();
	let block = scene
		.text(text)
		.and_then(canvist_model::Document::first_text_block)
		.expect("paragraph");
	let step = canvist_model::Step::InsertInline {
		block,
		offset: 0,
		content: canvist_model::InlineText::plain("x", MarkSet::empty()),
	};

	let not_text = Step::Text {
		node: rect,
		step: step.clone(),
	};
	assert_eq!(
		scene.apply_steps([not_text]),
		Err(SceneError::NotText(rect))
	);

	let doc = scene.lend_text(text)?;
	let lent = Step::Text { node: text, step };
	assert_eq!(
		scene.apply_steps([lent.clone()]),
		Err(SceneError::TextLent(text))
	);
	assert_eq!(
		scene.apply_steps([Step::RemoveNodes { ids: vec![text] }]),
		Err(SceneError::TextLent(text))
	);
	scene.return_text(text, doc)?;
	scene.apply_steps([lent])?;
	assert_eq!(
		scene
			.text(text)
			.map(canvist_model::Document::plain_text)
			.as_deref(),
		Some("x")
	);
	Ok(())
}

#[test]
fn equal_keys_order_by_id_and_inserts_go_after_the_run() -> Result<(), SceneError> {
	let mut scene = Scene::new(support::schema(), 1)?;
	let page = scene.pages()[0];
	let schema = scene.schema().clone();
	let rect = schema.node_type("rectangle").expect("standard type");
	let key = FracKey::parse("a5")?;
	let record = |client: u32, name: &str| {
		canvist_scene::NodeRecord::new(NodeId::new(client, 0), rect, page, key.clone()).with_props(
			Props {
				name: name.into(),
				shape: Shape::Rect,
				..Props::default()
			},
		)
	};

	// Two peers inserted at the same place: the id breaks the tie.
	scene.apply_steps([Step::InsertNodes {
		nodes: vec![record(9, "nine"), record(3, "three")],
	}])?;
	assert_eq!(names(&scene, page), ["three", "nine"]);

	let mut transaction = scene.transaction();
	transaction.insert(page, Some(1), "rectangle", named("after"))?;
	let _ = transaction.commit();
	assert_eq!(names(&scene, page), ["three", "nine", "after"]);
	Ok(())
}

#[test]
fn reads_the_nested_agent_form() -> Result<(), SceneError> {
	let json = json!({
		"type": "canvist-scene",
		"nodes": [{
			"type": "page",
			"props": { "name": "Poster" },
			"children": [{
				"type": "frame",
				"props": { "size": [400, 300], "fills": [{ "type": "solid", "color": "#f5f3ff" }] },
				"children": [
					{ "type": "ellipse", "props": { "size": [100, 100], "transform": [1, 0, 0, 1, 40, 40] } },
					{ "type": "path", "props": { "shape": { "type": "path", "d": "M0 0C10 0 20 10 20 20" } } },
					{ "type": "text", "text": { "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Hi" }] }] } }
				]
			}]
		}]
	});

	let scene = Scene::from_json(support::schema(), 7, &json)?;
	assert_eq!(scene.check(), Ok(()));
	let page = scene.pages()[0];
	let frame = scene.children(page)[0];
	let kinds: Vec<_> = scene
		.children(frame)
		.iter()
		.filter_map(|&id| scene.kind(id))
		.collect();

	assert_eq!(kinds, [NodeKind::Ellipse, NodeKind::Path, NodeKind::Text]);
	assert_eq!(
		scene.node(frame).map(|node| node.props().size),
		Some(Vec2::new(400.0, 300.0))
	);
	assert!(scene.node(frame).is_some_and(|node| node.props().clip));
	let text = scene.children(frame)[2];
	assert_eq!(
		scene
			.text(text)
			.map(canvist_model::Document::plain_text)
			.as_deref(),
		Some("Hi")
	);

	let canonical = scene.to_json();
	assert_eq!(
		Scene::from_json(support::schema(), 8, &canonical)?.to_json(),
		canonical
	);
	Ok(())
}

#[test]
fn json_errors_name_the_path() {
	let cases = [
		(json!({ "type": "nope" }), "type"),
		(
			json!({ "type": "canvist-scene", "nodes": [{ "type": "rectangle" }] }),
			"nodes[0].parent",
		),
		(
			json!({ "type": "canvist-scene", "nodes": [{ "type": "page", "props": { "opacity": "x" } }] }),
			"nodes[0].props.opacity",
		),
		(
			json!({ "type": "canvist-scene", "nodes": [{ "type": "page", "props": { "width": 3 } }] }),
			"nodes[0].props.width",
		),
	];

	for (json, path) in cases {
		match Scene::from_json(support::schema(), 1, &json) {
			Err(SceneError::Json { path: found, .. }) => assert_eq!(found, path),
			other => panic!("expected a JSON error at {path}, got {other:?}"),
		}
	}

	let unknown = json!({ "type": "canvist-scene", "nodes": [{ "type": "page", "children": [{ "type": "blob" }] }] });
	assert_eq!(
		Scene::from_json(support::schema(), 1, &unknown).err(),
		Some(SceneError::UnknownType("blob".into()))
	);
}

#[test]
fn custom_types_draw_their_own_geometry() -> Result<(), Box<dyn std::error::Error>> {
	let star = NodeSpec::custom("burst", false, |node| {
		let mut path = BezPath::new();
		let size = node.props().size;
		path.move_to((0.0, 0.0));
		path.line_to((size.x, size.y));
		path
	});
	let schema = Arc::new(
		SceneSchema::builder()
			.node(NodeSpec::new("frame", NodeKind::Frame))
			.node(NodeSpec::new("sticky", NodeKind::Frame))
			.node(star)
			.build()?,
	);
	let mut scene = Scene::new(schema.clone(), 1)?;
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let sticky = transaction.insert(page, None, "sticky", Props::default())?;
	let burst = transaction.insert(
		sticky,
		None,
		"burst",
		Props {
			size: Vec2::new(4.0, 2.0),
			..Props::default()
		},
	)?;
	let _ = transaction.commit();
	let node = scene.node(burst).expect("inserted");
	let geometry = scene
		.spec(burst)
		.and_then(NodeSpec::geometry)
		.expect("custom geometry");

	assert_eq!(geometry(node).elements().len(), 2);
	assert!(scene.text(sticky).is_none());
	assert!(
		SceneSchema::builder()
			.node(NodeSpec::new("text", NodeKind::Text))
			.build()
			.is_err()
	);
	assert!(
		SceneSchema::builder()
			.node(NodeSpec::new("page", NodeKind::Page))
			.build()
			.is_err()
	);
	Ok(())
}
