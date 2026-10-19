//! Canvas scenes, behind the `scene` feature: `WasmSceneEngine` and
//! `WasmSceneDoc`, which `@canvist/headless/scene` wraps.

use canvist_headless::ExportOptions;
use canvist_headless::HeadlessError;
use canvist_headless::SceneDoc;
use canvist_headless::SceneEngine;
use canvist_headless::canvist_scene::NodeId;
use js_sys::Array;
use serde_json::Value as Json;
use wasm_bindgen::prelude::*;

use crate::Result;
use crate::bytes;
use crate::error;
use crate::parse_json;

fn invalid(name: &str, reason: impl Into<String>) -> JsValue {
	error(HeadlessError::InvalidArgs {
		name: name.to_owned(),
		reason: reason.into(),
	})
}

fn node_ids(ids: &[String]) -> Result<Vec<NodeId>> {
	ids.iter()
		.map(|id| {
			id.parse()
				.map_err(|_| invalid("select", format!("`{id}` is not a node id")))
		})
		.collect()
}

/// `ExportOptions` from JSON: `{ nodes?: string[], scale?: number,
/// padding?: number }`.
fn export_options(text: &str) -> Result<ExportOptions> {
	let json = parse_json(text)?;
	let defaults = ExportOptions::default();
	let number = |key: &str, default: f64| {
		match json.get(key) {
			None | Some(Json::Null) => Ok(default),
			Some(value) => {
				value
					.as_f64()
					.ok_or_else(|| invalid("export", format!("`{key}` must be a number")))
			}
		}
	};
	let nodes: Vec<String> = match json.get("nodes") {
		None | Some(Json::Null) => Vec::new(),
		Some(value) => {
			serde_json::from_value(value.clone())
				.map_err(|_| invalid("export", "`nodes` must be an array of node ids"))?
		}
	};

	Ok(ExportOptions {
		nodes: node_ids(&nodes)?,
		scale: number("scale", defaults.scale)?,
		padding: number("padding", defaults.padding)?,
	})
}

/// A configured set of canvas extensions that creates scenes.
#[wasm_bindgen(js_name = WasmSceneEngine)]
pub struct WasmSceneEngine {
	engine: SceneEngine,
}

#[wasm_bindgen(js_class = WasmSceneEngine)]
impl WasmSceneEngine {
	/// An engine with every built-in canvas extension, allocating node ids
	/// as `client`.
	#[wasm_bindgen(constructor)]
	#[must_use]
	pub fn new(client: u32) -> WasmSceneEngine {
		Self {
			engine: SceneEngine::design_kit().with_client(client),
		}
	}

	/// Tools, commands, node types, and inspector sections, as JSON.
	#[must_use]
	pub fn metadata(&self) -> String {
		self.engine.metadata().to_string()
	}

	/// A scene with one empty page.
	pub fn create(&self) -> Result<WasmSceneDoc> {
		self.engine.create().map(WasmSceneDoc::from).map_err(error)
	}

	/// A scene from scene JSON text, flat or nested.
	#[wasm_bindgen(js_name = fromJson)]
	pub fn load_json(&self, json: &str) -> Result<WasmSceneDoc> {
		self.engine
			.from_json(&parse_json(json)?)
			.map(WasmSceneDoc::from)
			.map_err(error)
	}

	/// A scene joined from a peer's v1 Yjs state.
	#[wasm_bindgen(js_name = fromYjs)]
	pub fn join_yjs(&self, state: &[u8]) -> Result<WasmSceneDoc> {
		self.engine
			.from_yjs(state)
			.map(WasmSceneDoc::from)
			.map_err(error)
	}
}

/// One scene with a canvas around it.
#[wasm_bindgen(js_name = WasmSceneDoc)]
pub struct WasmSceneDoc {
	doc: SceneDoc,
}

impl From<SceneDoc> for WasmSceneDoc {
	fn from(doc: SceneDoc) -> Self {
		Self { doc }
	}
}

#[wasm_bindgen(js_class = WasmSceneDoc)]
impl WasmSceneDoc {
	/// The scene as canonical JSON text.
	#[wasm_bindgen(js_name = toJson)]
	#[must_use]
	pub fn to_json(&self) -> String {
		self.doc.to_json().to_string()
	}

	/// Run canvas command `name` with JSON `args` against the selection.
	pub fn run(&mut self, name: &str, args: &str) -> Result<()> {
		self.doc.run(name, &parse_json(args)?).map_err(error)
	}

	/// Select the nodes with these ids.
	#[allow(
		clippy::needless_pass_by_value,
		reason = "wasm-bindgen hands JavaScript arrays over by value"
	)]
	pub fn select(&mut self, ids: Vec<String>) -> Result<()> {
		self.doc.select(&node_ids(&ids)?).map_err(error)
	}

	/// The selected nodes' ids.
	#[must_use]
	pub fn selection(&self) -> Vec<String> {
		self.doc
			.selection()
			.iter()
			.map(ToString::to_string)
			.collect()
	}

	/// The inspector for the selection, as JSON.
	#[must_use]
	pub fn inspect(&self) -> String {
		self.doc.inspect().to_string()
	}

	/// Set inspector field `key` to the JSON `value` on every selected node.
	#[wasm_bindgen(js_name = setField)]
	pub fn set_field(&mut self, key: &str, value: &str) -> Result<()> {
		self.doc.set_field(key, &parse_json(value)?).map_err(error)
	}

	/// Undo the latest change; returns whether there was one.
	pub fn undo(&mut self) -> Result<bool> {
		self.doc.undo().map_err(error)
	}

	/// Provide image asset `asset`'s pixels, `width` by `height` RGBA8.
	#[wasm_bindgen(js_name = addImage)]
	pub fn add_image(&mut self, asset: &str, width: u32, height: u32, rgba: Vec<u8>) -> Result<()> {
		self.doc
			.add_image(asset, width, height, rgba)
			.map_err(error)
	}

	/// An SVG document; `options` is JSON export options.
	#[wasm_bindgen(js_name = exportSvg)]
	pub fn export_svg(&self, options: &str) -> Result<String> {
		self.doc
			.export_svg(&export_options(options)?)
			.map_err(error)
	}

	/// A PNG image; `options` is JSON export options.
	#[wasm_bindgen(js_name = exportPng)]
	pub fn export_png(&self, options: &str) -> Result<Vec<u8>> {
		self.doc
			.export_png(&export_options(options)?)
			.map_err(error)
	}

	/// Share the scene as yrs client `client_id`.
	pub fn share(&mut self, client_id: u32) -> Result<()> {
		self.doc.share(u64::from(client_id)).map_err(error)
	}

	/// The whole shared scene as one v1 Yjs update.
	#[wasm_bindgen(js_name = encodeState)]
	pub fn encode_state(&self) -> Result<Vec<u8>> {
		self.doc.encode_state().map_err(error)
	}

	/// Merge a peer's v1 update; returns whether the scene changed.
	#[wasm_bindgen(js_name = applyUpdate)]
	pub fn apply_update(&mut self, update: &[u8]) -> Result<bool> {
		self.doc.apply_update(update).map_err(error)
	}

	/// The v1 updates produced since the last call.
	#[wasm_bindgen(js_name = takeUpdates)]
	pub fn take_updates(&mut self) -> Array {
		bytes(self.doc.take_updates())
	}
}
