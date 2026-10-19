//! # `canvist_headless_wasm`
//!
//! WebAssembly bindings for [`canvist_headless`], for JavaScript runtimes
//! with no DOM: Node, Deno, Bun, and Workers. It uses no `web-sys`, only
//! `js-sys` for errors and arrays.
//!
//! The `@canvist/headless` package wraps these classes in a typed API. The
//! surface here is deliberately plain: JSON travels as strings, bytes as
//! `Uint8Array`s, and every failure is an `Error` named `CanvistError` whose
//! `code` property is the [`ErrorCode`](canvist_headless::ErrorCode) name.

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use canvist_headless::Snapshot;
use js_sys::Array;
use js_sys::Uint8Array;
use serde_json::Value as Json;
use serde_json::json;
use wasm_bindgen::prelude::*;

type Result<T> = std::result::Result<T, JsValue>;

/// A `CanvistError` carrying the error's stable code.
#[allow(
	clippy::needless_pass_by_value,
	reason = "a `map_err` callback receives the error by value"
)]
fn error(error: HeadlessError) -> JsValue {
	let js = js_sys::Error::new(&error.to_string());
	js.set_name("CanvistError");
	// Setting a property on a fresh `Error` object cannot fail.
	let _ = js_sys::Reflect::set(&js, &"code".into(), &error.code().as_str().into());
	js.into()
}

fn invalid_json(reason: &serde_json::Error) -> JsValue {
	error(HeadlessError::InvalidArgs {
		name: "json".to_owned(),
		reason: reason.to_string(),
	})
}

fn parse_json(text: &str) -> Result<Json> {
	serde_json::from_str(text).map_err(|reason| invalid_json(&reason))
}

fn format(name: &str) -> Result<Format> {
	name.parse().map_err(error)
}

#[allow(
	clippy::needless_pass_by_value,
	reason = "a `map` callback receives the chunks by value"
)]
fn bytes(chunks: Vec<Vec<u8>>) -> Array {
	chunks
		.iter()
		.map(|chunk| Uint8Array::from(chunk.as_slice()))
		.collect()
}

/// A configured set of extensions that creates documents.
#[wasm_bindgen(js_name = WasmEngine)]
pub struct WasmEngine {
	engine: Engine,
}

#[wasm_bindgen(js_class = WasmEngine)]
impl WasmEngine {
	/// An engine with the named extensions, allocating block ids as
	/// `client`.
	#[wasm_bindgen(constructor)]
	#[allow(
		clippy::needless_pass_by_value,
		reason = "wasm-bindgen hands JavaScript arrays over by value"
	)]
	pub fn new(extensions: Vec<String>, client: u32) -> Result<WasmEngine> {
		let engine = Engine::named(&extensions).map_err(error)?;

		Ok(Self {
			engine: engine.with_client(client),
		})
	}

	/// The registered extensions' metadata as JSON.
	pub fn metadata(&self) -> String {
		self.engine.metadata().to_string()
	}

	/// A document with one empty paragraph.
	pub fn create(&self) -> Result<WasmDoc> {
		self.engine.create().map(WasmDoc::from).map_err(error)
	}

	/// Read `input` as `format` (`json`, `html`, `markdown`, or `text`).
	pub fn parse(&self, format_name: &str, input: &str) -> Result<WasmDoc> {
		let format = format(format_name)?;
		self.engine
			.parse(format, input)
			.map(WasmDoc::from)
			.map_err(error)
	}

	/// Join a shared document from v1 Yjs state as yrs client `client_id`.
	#[wasm_bindgen(js_name = fromYjs)]
	pub fn from_yjs(&self, state: &[u8], client_id: u32) -> Result<WasmDoc> {
		self.engine
			.from_yjs(state, u64::from(client_id))
			.map(WasmDoc::from)
			.map_err(error)
	}

	/// Recreate a document from a snapshot's JSON text.
	pub fn restore(&self, snapshot: &str) -> Result<WasmDoc> {
		let snapshot = Snapshot::from_json(&parse_json(snapshot)?).map_err(error)?;
		self.engine
			.restore(&snapshot)
			.map(WasmDoc::from)
			.map_err(error)
	}
}

/// One document and its editor.
#[wasm_bindgen(js_name = WasmDoc)]
pub struct WasmDoc {
	doc: Doc,
}

impl From<Doc> for WasmDoc {
	fn from(doc: Doc) -> Self {
		Self { doc }
	}
}

#[wasm_bindgen(js_class = WasmDoc)]
impl WasmDoc {
	/// The document as `format`.
	pub fn export(&self, format_name: &str) -> Result<String> {
		Ok(self.doc.export(format(format_name)?))
	}

	/// The anchor and head as JSON positions, or `null` for a block
	/// selection.
	pub fn selection(&self) -> String {
		let range = self.doc.selected_range();
		let side =
			|position: &Position| json!({ "path": position.path, "offset": position.offset });
		range
			.map_or(
				Json::Null,
				|(anchor, head)| json!({ "anchor": side(&anchor), "head": side(&head) }),
			)
			.to_string()
	}

	/// Select from the anchor to the head; offsets count UTF-16 code units.
	pub fn select(
		&mut self,
		anchor_path: Vec<u32>,
		anchor_offset: u32,
		head_path: Vec<u32>,
		head_offset: u32,
	) -> Result<()> {
		let anchor = Position::new(anchor_path, anchor_offset);
		let head = Position::new(head_path, head_offset);
		self.doc.select_range(&anchor, &head).map_err(error)
	}

	/// Select the whole document.
	#[wasm_bindgen(js_name = selectAll)]
	pub fn select_all(&mut self) -> Result<()> {
		self.doc.select_all().map_err(error)
	}

	/// Replace the selection with `text`, without input rules.
	#[wasm_bindgen(js_name = insertText)]
	pub fn insert_text(&mut self, text: &str) -> Result<bool> {
		self.doc.insert_text(text).map_err(error)
	}

	/// Type `text` one character at a time, running input rules.
	#[wasm_bindgen(js_name = typeText)]
	pub fn type_text(&mut self, text: &str) -> Result<bool> {
		self.doc.type_text(text).map_err(error)
	}

	/// Delete the selection.
	#[wasm_bindgen(js_name = deleteSelection)]
	pub fn delete_selection(&mut self) -> Result<bool> {
		self.doc.delete_selection().map_err(error)
	}

	/// Press a key binding such as `Enter` or `Mod-b`.
	pub fn press(&mut self, key: &str) -> Result<bool> {
		self.doc.press(key).map_err(error)
	}

	/// Paste `content` read as `format` over the selection.
	pub fn paste(&mut self, format_name: &str, content: &str) -> Result<bool> {
		let format = format(format_name)?;
		self.doc.paste(format, content).map_err(error)
	}

	/// Run the named command with JSON arguments.
	pub fn run(&mut self, name: &str, args: &str) -> Result<bool> {
		let args = parse_json(args)?;
		self.doc.run_named(name, &args).map_err(error)
	}

	/// Whether the named command would apply.
	pub fn can(&mut self, name: &str, args: &str) -> Result<bool> {
		let args = parse_json(args)?;
		self.doc.can_named(name, &args).map_err(error)
	}

	/// Answer the named query, as JSON.
	pub fn query(&self, name: &str, args: &str) -> Result<String> {
		let args = parse_json(args)?;
		let answer = self.doc.query(name, &args).map_err(error)?;
		Ok(answer.to_string())
	}

	/// Undo the last call that changed the document.
	pub fn undo(&mut self) -> Result<bool> {
		self.doc.undo().map_err(error)
	}

	/// Redo the last undone call.
	pub fn redo(&mut self) -> Result<bool> {
		self.doc.redo().map_err(error)
	}

	/// The content and selection as snapshot JSON.
	pub fn snapshot(&self) -> String {
		self.doc.snapshot().to_json().to_string()
	}

	/// Start sharing the document as yrs client `client_id`.
	pub fn share(&mut self, client_id: u32) -> Result<()> {
		self.doc.share(u64::from(client_id)).map_err(error)
	}

	/// Apply a v1 Yjs update; returns whether the document changed.
	#[wasm_bindgen(js_name = applyUpdate)]
	pub fn apply_update(&mut self, update: &[u8]) -> Result<bool> {
		self.doc.apply_update(update).map_err(error)
	}

	/// The whole shared state as one v1 update.
	#[wasm_bindgen(js_name = encodeState)]
	pub fn encode_state(&self) -> Result<Vec<u8>> {
		self.doc.encode_state().map_err(error)
	}

	/// The v1 state vector.
	#[wasm_bindgen(js_name = stateVector)]
	pub fn state_vector(&self) -> Result<Vec<u8>> {
		self.doc.state_vector().map_err(error)
	}

	/// What a peer at `state_vector` is missing, as one v1 update.
	#[wasm_bindgen(js_name = encodeDiff)]
	pub fn encode_diff(&self, state_vector: &[u8]) -> Result<Vec<u8>> {
		self.doc.encode_diff(state_vector).map_err(error)
	}

	/// Drain this peer's own v1 updates.
	#[wasm_bindgen(js_name = takeUpdates, unchecked_return_type = "Uint8Array[]")]
	pub fn take_updates(&mut self) -> Result<Array> {
		self.doc.take_updates().map(bytes).map_err(error)
	}

	/// `{ rejectedBlocks, waiting }` as JSON.
	pub fn validate(&self) -> Result<String> {
		let validation = self.doc.validate().map_err(error)?;
		let report = json!({
			"rejectedBlocks": validation.rejected_blocks,
			"waiting": validation.waiting,
		});
		Ok(report.to_string())
	}

	/// The y-sync message to greet a connecting client with.
	#[wasm_bindgen(js_name = syncStart)]
	pub fn sync_start(&self) -> Result<Vec<u8>> {
		self.doc.sync_start().map_err(error)
	}

	/// Handle a y-sync message: returns `[reply, broadcast, changed]`, the
	/// messages for the sender, the messages for everyone else, and whether
	/// the document changed.
	#[wasm_bindgen(js_name = receiveSync, unchecked_return_type = "[Uint8Array[], Uint8Array[], boolean]")]
	pub fn receive_sync(&mut self, message: &[u8]) -> Result<Array> {
		let sync = self.doc.receive_sync(message).map_err(error)?;
		Ok(Array::of3(
			&bytes(sync.reply),
			&bytes(sync.broadcast),
			&sync.changed.into(),
		))
	}

	/// This peer's changes since the last call, as y-sync update messages.
	#[wasm_bindgen(js_name = flushSync, unchecked_return_type = "Uint8Array[]")]
	pub fn flush_sync(&mut self) -> Result<Array> {
		self.doc.flush_sync().map(bytes).map_err(error)
	}
}
