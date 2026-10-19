//! Canvas scenes on a server: load, transform, and export.

#![cfg(feature = "scene")]

use canvist_headless::ErrorCode;
use canvist_headless::ExportOptions;
use canvist_headless::HeadlessError;
use canvist_headless::SceneDoc;
use canvist_headless::SceneEngine;
use canvist_headless::canvist_scene::Node;
use canvist_headless::canvist_scene::NodeId;
use canvist_headless::canvist_scene::Props;
use serde_json::Value as Json;
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The poster's card frame: where it sits and how big it is.
const CARD: [f64; 4] = [40.0, 30.0, 300.0, 200.0];
/// Bytes before a PNG's width in its header: the signature, the chunk
/// length, and the `IHDR` tag.
const PNG_WIDTH_AT: usize = 16;
/// The PNG file signature.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A small poster in the nested scene JSON an agent would write.
fn poster() -> Json {
	let [x, y, width, height] = CARD;
	json!({
		"type": "canvist-scene",
		"version": 1,
		"nodes": [{
			"type": "page",
			"props": { "fills": [{ "type": "solid", "color": "#f6f3ec" }] },
			"children": [{
				"type": "frame",
				"props": {
					"name": "Card",
					"size": [width, height],
					"transform": [1, 0, 0, 1, x, y],
					"fills": [{ "type": "solid", "color": "#ffffff" }]
				},
				"children": [
					{
						"type": "ellipse",
						"props": {
							"name": "Sun",
							"size": [80, 80],
							"transform": [1, 0, 0, 1, 20, 20],
							"fills": [{ "type": "solid", "color": "#ffc83d" }]
						}
					},
					{
						"type": "text",
						"props": { "name": "Title", "size": [200, 0], "transform": [1, 0, 0, 1, 20, 120] },
						"text": {
							"type": "doc",
							"content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Summer sale" }] }]
						}
					},
					{
						"type": "image",
						"props": {
							"name": "Photo",
							"size": [60, 60],
							"transform": [1, 0, 0, 1, 220, 20],
							"fills": [{ "type": "image", "asset": "photo.png" }]
						}
					}
				]
			}]
		}]
	})
}

fn doc() -> Result<SceneDoc, HeadlessError> {
	SceneEngine::design_kit().from_json(&poster())
}

fn named(doc: &SceneDoc, name: &str) -> Option<NodeId> {
	let scene = doc.scene();
	scene
		.nodes()
		.find(|node| &*node.props().name == name)
		.map(Node::id)
}

/// A PNG's width and height, from its header.
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
	let header = png.get(PNG_WIDTH_AT..PNG_WIDTH_AT + 8)?;
	let width = u32::from_be_bytes(header[..4].try_into().ok()?);
	let height = u32::from_be_bytes(header[4..].try_into().ok()?);
	Some((width, height))
}

#[test]
fn a_scene_round_trips_through_json() -> TestResult {
	let engine = SceneEngine::design_kit();
	let doc = engine.from_json(&poster())?;
	let again = engine.from_json(&doc.to_json())?;

	assert_eq!(again.to_json(), doc.to_json());
	// The page, the card, and its three children.
	assert_eq!(doc.scene().len(), 5);
	Ok(())
}

#[test]
fn commands_and_inspector_fields_transform_a_scene() -> TestResult {
	let mut doc = doc()?;
	let sun = named(&doc, "Sun").ok_or("the poster has a sun")?;

	doc.select(&[sun])?;
	doc.set_field("name", &json!("Moon"))?;
	doc.set_field("x", &json!(100))?;
	assert_eq!(field(&doc.inspect(), "x"), Some(json!(100.0)));
	assert!(named(&doc, "Moon").is_some());

	doc.run(
		"insert",
		&json!({ "type": "rectangle", "props": { "name": "Badge", "size": [40, 20] } }),
	)?;
	assert!(named(&doc, "Badge").is_some());

	assert!(doc.undo()?);
	assert!(named(&doc, "Badge").is_none());
	Ok(())
}

#[test]
fn the_page_exports_as_svg_with_text_and_images() -> TestResult {
	let mut doc = doc()?;
	let placeholder = doc.export_svg(&ExportOptions::default())?;
	assert!(
		!placeholder.contains("photo.png"),
		"an image without pixels is a placeholder"
	);

	doc.add_image("photo.png", 1, 1, vec![u8::MAX; 4])?;
	let svg = doc.export_svg(&ExportOptions::default())?;

	assert!(svg.starts_with("<svg"));
	// The page's content is the card, so the image is the card's size.
	let [.., width, height] = CARD;
	assert!(
		svg.contains(&format!("width=\"{width}\" height=\"{height}\"")),
		"{svg}"
	);
	assert!(svg.contains("Summer"), "text stays text");
	assert!(svg.contains("photo.png"), "images reference their asset id");
	Ok(())
}

#[test]
fn a_node_exports_as_png_at_a_scale_with_padding() -> TestResult {
	let mut doc = doc()?;
	let side = 4;
	doc.add_image(
		"photo.png",
		side,
		side,
		vec![u8::MAX; (side * side * 4) as usize],
	)?;
	let sun = named(&doc, "Sun").ok_or("the poster has a sun")?;
	let options = ExportOptions {
		nodes: vec![sun],
		scale: 2.0,
		padding: 5.0,
	};

	let png = doc.export_png(&options)?;

	assert!(png.starts_with(PNG_SIGNATURE));
	// An 80-unit sun, padded by 5 on each side, at twice the size.
	assert_eq!(png_size(&png), Some((180, 180)));
	assert!(
		doc.export_png(&ExportOptions::default())?
			.starts_with(PNG_SIGNATURE)
	);
	Ok(())
}

#[test]
fn failures_carry_stable_codes() -> TestResult {
	let engine = SceneEngine::design_kit();
	let mut doc = engine.create()?;
	let code = |result: Result<(), HeadlessError>| result.err().map(|error| error.code());

	assert_eq!(
		code(doc.run("teleport", &json!({}))),
		Some(ErrorCode::UnknownName)
	);
	assert_eq!(
		code(doc.run("insert", &json!({ "type": "hexagon" }))),
		Some(ErrorCode::InvalidArgs)
	);
	// A rectangle cannot hold children.
	let page = doc.scene().pages()[0];
	let nested = doc.edit(|transaction| {
		let rect = transaction.insert(page, None, "rectangle", Props::default())?;
		transaction.insert(rect, None, "rectangle", Props::default())
	});
	assert_eq!(code(nested.map(drop)), Some(ErrorCode::Schema));
	assert_eq!(
		code(
			engine
				.from_json(&json!({ "type": "canvist-scene" }))
				.map(drop)
		),
		Some(ErrorCode::Parse)
	);
	assert_eq!(
		code(doc.export_svg(&ExportOptions::default()).map(drop)),
		Some(ErrorCode::InvalidArgs),
		"an empty page has nothing to draw"
	);
	assert_eq!(
		code(doc.add_image("short", 2, 2, vec![0; 3])),
		Some(ErrorCode::InvalidArgs)
	);
	Ok(())
}

#[cfg(feature = "collab")]
#[test]
fn a_shared_scene_syncs_with_a_peer() -> TestResult {
	let engine = SceneEngine::design_kit();
	let mut ada = engine.from_json(&poster())?;
	ada.share(1)?;
	let mut grace = SceneEngine::design_kit()
		.with_client(2)
		.from_yjs(&ada.encode_state()?)?;
	assert_eq!(grace.to_json(), ada.to_json());

	let sun = named(&ada, "Sun").ok_or("the poster has a sun")?;
	ada.select(&[sun])?;
	ada.set_field("name", &json!("Moon"))?;

	let mut changed = false;

	for update in ada.take_updates() {
		changed |= grace.apply_update(&update)?;
	}

	assert!(changed);

	assert!(named(&grace, "Moon").is_some());
	assert_eq!(grace.to_json(), ada.to_json());
	assert!(matches!(ada.share(1), Err(HeadlessError::AlreadyShared)));
	Ok(())
}

/// The value of inspector field `key`, from whichever section holds it.
fn field(inspector: &Json, key: &str) -> Option<Json> {
	inspector["sections"]
		.as_array()?
		.iter()
		.filter_map(|section| section["fields"].as_array())
		.flatten()
		.find(|field| field["key"] == key)
		.map(|field| field["value"].clone())
}
