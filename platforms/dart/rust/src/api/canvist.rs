//! Engines and documents for Dart.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use canvist_headless::Renderer;
use canvist_headless::Snapshot;
use canvist_headless::canvist_core::Platform;
use flutter_rust_bridge::frb;
use serde_json::Value as Json;

use crate::api::view::CanvistView;
use crate::api::view::ViewDisplay;
use crate::api::view::ViewFont;
use crate::api::view::ViewOptions;
use crate::measure::HostMeasurer;

/// A failure inside canvist. `code` is the stable
/// [`ErrorCode`](canvist_headless::ErrorCode) name every binding shares.
#[derive(Debug)]
pub struct CanvistFailure {
	/// The stable code, such as `invalidArgs`.
	pub code: String,
	/// What went wrong.
	pub message: String,
}

impl From<HeadlessError> for CanvistFailure {
	fn from(error: HeadlessError) -> Self {
		Self {
			code: error.code().as_str().to_owned(),
			message: error.to_string(),
		}
	}
}

type Result<T> = std::result::Result<T, CanvistFailure>;

/// A format documents are read from and written to.
#[derive(Clone, Copy, Debug)]
pub enum DocFormat {
	/// ProseMirror and TipTap JSON.
	Json,
	/// Semantic HTML.
	Html,
	/// Markdown.
	Markdown,
	/// Plain text.
	Text,
}

impl From<DocFormat> for Format {
	fn from(format: DocFormat) -> Self {
		match format {
			DocFormat::Json => Self::Json,
			DocFormat::Html => Self::Html,
			DocFormat::Markdown => Self::Markdown,
			DocFormat::Text => Self::Text,
		}
	}
}

/// A point in a text block: child indexes from the root, and UTF-16 code
/// units into its text.
#[derive(Clone, Debug)]
pub struct DocPosition {
	/// Child indexes from the root to the text block.
	pub path: Vec<u32>,
	/// UTF-16 code units into the block's text.
	pub offset: u32,
}

impl From<DocPosition> for Position {
	fn from(position: DocPosition) -> Self {
		Self::new(position.path, position.offset)
	}
}

impl From<Position> for DocPosition {
	fn from(position: Position) -> Self {
		Self {
			path: position.path,
			offset: position.offset,
		}
	}
}

/// What to send after handling a y-sync message.
#[derive(Debug)]
pub struct DocSyncReply {
	/// Messages for the connection the message came from.
	pub reply: Vec<Vec<u8>>,
	/// Messages for every other connection.
	pub broadcast: Vec<Vec<u8>>,
	/// Whether the document changed.
	pub changed: bool,
}

/// How well the shared state fits the schema.
#[derive(Debug)]
pub struct DocValidation {
	/// Shared items the schema rejects.
	pub rejected_blocks: u32,
	/// Whether updates wait for earlier ones.
	pub waiting: bool,
}

/// Which platform's key bindings an engine follows.
#[derive(Clone, Copy, Debug)]
pub enum DocPlatform {
	/// macOS, iOS, and iPadOS: `Mod` is Cmd.
	Apple,
	/// Everything else: `Mod` is Ctrl.
	Other,
}

impl From<DocPlatform> for Platform {
	fn from(platform: DocPlatform) -> Self {
		match platform {
			DocPlatform::Apple => Self::Apple,
			DocPlatform::Other => Self::Other,
		}
	}
}

/// HTML for a document, with the stylesheet that lays it out as the
/// editor does.
#[derive(Debug)]
pub struct DocHtml {
	/// The document's HTML fragment.
	pub html: String,
	/// The stylesheet.
	pub css: String,
}

thread_local! {
	/// Server rendering caches measurements, so each thread keeps one.
	static RENDERER: RefCell<Renderer> = RefCell::new(Renderer::new());
}

fn json(text: &str) -> Result<Json> {
	serde_json::from_str(text).map_err(|error| {
		CanvistFailure::from(HeadlessError::InvalidArgs {
			name: "json".to_owned(),
			reason: error.to_string(),
		})
	})
}

/// A configured set of extensions that creates documents.
#[frb(opaque)]
pub struct CanvistEngine {
	engine: Engine,
}

impl CanvistEngine {
	/// An engine with the named StarterKit extensions, allocating block ids
	/// as `client`.
	#[frb(sync)]
	pub fn new(extensions: Vec<String>, client: u32, platform: DocPlatform) -> Result<Self> {
		let engine = Engine::named(&extensions)?
			.with_client(client)
			.with_platform(platform.into());
		Ok(Self { engine })
	}

	/// The registered extensions' metadata as JSON.
	#[frb(sync)]
	pub fn metadata(&self) -> String {
		self.engine.metadata().to_string()
	}

	/// A document with one empty paragraph.
	#[frb(sync)]
	pub fn create(&self) -> Result<CanvistDoc> {
		Ok(CanvistDoc::from(self.engine.create()?))
	}

	/// Read `input` as `format`.
	#[frb(sync)]
	pub fn parse(&self, format: DocFormat, input: &str) -> Result<CanvistDoc> {
		Ok(CanvistDoc::from(self.engine.parse(format.into(), input)?))
	}

	/// Join a shared document from v1 Yjs state as yrs client `client_id`.
	#[frb(sync)]
	pub fn from_yjs(&self, state: &[u8], client_id: u32) -> Result<CanvistDoc> {
		let doc = self.engine.from_yjs(state, u64::from(client_id))?;
		Ok(CanvistDoc::from(doc))
	}

	/// Recreate a document from snapshot JSON.
	#[frb(sync)]
	pub fn restore(&self, snapshot: &str) -> Result<CanvistDoc> {
		let snapshot = Snapshot::from_json(&json(snapshot)?)?;
		Ok(CanvistDoc::from(self.engine.restore(&snapshot)?))
	}
}

/// One document and its editor.
#[frb(opaque)]
pub struct CanvistDoc {
	// The bridge shares opaque values between isolates, so they must be
	// `Sync`; a `Doc` is only `Send`. A view shares the document.
	doc: Arc<Mutex<Doc>>,
}

impl From<Doc> for CanvistDoc {
	fn from(doc: Doc) -> Self {
		Self {
			doc: Arc::new(Mutex::new(doc)),
		}
	}
}

impl CanvistDoc {
	fn lock(&self) -> MutexGuard<'_, Doc> {
		// A panic while holding the lock leaves the document as the failed
		// call left it, which every call is written to tolerate.
		self.doc
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
	}

	/// A view laying the document out for a host to paint and edit,
	/// measuring text through the Dart function at address `measurer`: a
	/// `NativeCallable.isolateLocal` with the C signature
	/// `void measure(const MeasureRequest*)`, which must stay open while the
	/// view lives. The `canvist` package's `EditorView` is the only caller.
	#[frb(sync)]
	pub fn view(&self, measurer: usize, options: ViewOptions) -> Result<CanvistView> {
		#[allow(unsafe_code, reason = "the measurer comes from the Dart package")]
		// SAFETY: the Dart `EditorView` passes the address of its own
		// isolate-local callable and closes it only after disposing the view,
		// and every view call runs synchronously on that isolate's thread.
		let measurer = unsafe { HostMeasurer::new(measurer) }.ok_or_else(|| {
			CanvistFailure::from(HeadlessError::InvalidArgs {
				name: "view".to_owned(),
				reason: "the measurer address is null".to_owned(),
			})
		})?;
		Ok(CanvistView::new(self.doc.clone(), measurer, &options))
	}

	/// HTML and CSS that lay the document out `width` pixels wide as the
	/// editor does, for server rendering.
	#[frb(sync)]
	pub fn to_html(&self, width: f32) -> DocHtml {
		let doc = self.lock();
		let html = RENDERER.with_borrow(|renderer| renderer.html(&doc, width));
		DocHtml {
			html: html.html,
			css: html.css,
		}
	}

	/// The whole document as an SVG image `width` pixels wide, measured
	/// with the bundled Atkinson Hyperlegible font.
	#[frb(sync)]
	pub fn to_svg(&self, width: f32) -> Result<String> {
		let doc = self.lock();
		Ok(RENDERER.with_borrow_mut(|renderer| renderer.svg(&doc, width))?)
	}

	/// The display list [`CanvistDoc::to_svg`] paints, for hosts that check
	/// their own rendering against server rendering.
	#[frb(sync)]
	pub fn server_display(&self, width: f32) -> Result<ViewDisplay> {
		let doc = self.lock();
		let list = RENDERER.with_borrow_mut(|renderer| renderer.display_list(&doc, width))?;
		let theme = canvist_layout::Theme::starter_kit();

		Ok(ViewDisplay {
			fonts: list
				.fonts
				.iter()
				.map(|font| {
					ViewFont {
						family: font.family.to_string(),
						size: font.size,
						weight: font.weight,
						italic: font.italic,
					}
				})
				.collect(),
			items: list
				.items
				.iter()
				.map(|item| crate::api::view::item(&list, item))
				.collect(),
			background: crate::api::view::argb(theme.chrome().background),
		})
	}

	/// The document as `format`.
	#[frb(sync)]
	pub fn export(&self, format: DocFormat) -> String {
		self.lock().export(format.into())
	}

	/// The anchor and head of a text selection, or nothing for a block
	/// selection.
	#[frb(sync)]
	pub fn selection(&self) -> Option<Vec<DocPosition>> {
		let (anchor, head) = self.lock().selected_range()?;
		Some(vec![anchor.into(), head.into()])
	}

	/// Select from `anchor` to `head`.
	#[frb(sync)]
	pub fn select(&self, anchor: DocPosition, head: DocPosition) -> Result<()> {
		Ok(self.lock().select_range(&anchor.into(), &head.into())?)
	}

	/// Select the whole document.
	#[frb(sync)]
	pub fn select_all(&self) -> Result<()> {
		Ok(self.lock().select_all()?)
	}

	/// Replace the selection with `text`, without input rules.
	#[frb(sync)]
	pub fn insert_text(&self, text: &str) -> Result<bool> {
		Ok(self.lock().insert_text(text)?)
	}

	/// Type `text` one character at a time, running input rules.
	#[frb(sync)]
	pub fn type_text(&self, text: &str) -> Result<bool> {
		Ok(self.lock().type_text(text)?)
	}

	/// Delete the selection.
	#[frb(sync)]
	pub fn delete_selection(&self) -> Result<bool> {
		Ok(self.lock().delete_selection()?)
	}

	/// Press a key binding such as `Enter` or `Mod-b`.
	#[frb(sync)]
	pub fn press(&self, key: &str) -> Result<bool> {
		Ok(self.lock().press(key)?)
	}

	/// Paste `content` read as `format` over the selection.
	#[frb(sync)]
	pub fn paste(&self, format: DocFormat, content: &str) -> Result<bool> {
		Ok(self.lock().paste(format.into(), content)?)
	}

	/// Run the named command with JSON arguments.
	#[frb(sync)]
	pub fn run(&self, name: &str, args: &str) -> Result<bool> {
		Ok(self.lock().run_named(name, &json(args)?)?)
	}

	/// Whether the named command would apply.
	#[frb(sync)]
	pub fn can(&self, name: &str, args: &str) -> Result<bool> {
		Ok(self.lock().can_named(name, &json(args)?)?)
	}

	/// Answer the named query, as JSON.
	#[frb(sync)]
	pub fn query(&self, name: &str, args: &str) -> Result<String> {
		Ok(self.lock().query(name, &json(args)?)?.to_string())
	}

	/// Undo the last call that changed the document.
	#[frb(sync)]
	pub fn undo(&self) -> Result<bool> {
		Ok(self.lock().undo()?)
	}

	/// Redo the last undone call.
	#[frb(sync)]
	pub fn redo(&self) -> Result<bool> {
		Ok(self.lock().redo()?)
	}

	/// The content and selection as snapshot JSON.
	#[frb(sync)]
	pub fn snapshot(&self) -> String {
		self.lock().snapshot().to_json().to_string()
	}

	/// Start sharing the document as yrs client `client_id`.
	#[frb(sync)]
	pub fn share(&self, client_id: u32) -> Result<()> {
		Ok(self.lock().share(u64::from(client_id))?)
	}

	/// Apply a v1 Yjs update; returns whether the document changed.
	#[frb(sync)]
	pub fn apply_update(&self, update: &[u8]) -> Result<bool> {
		Ok(self.lock().apply_update(update)?)
	}

	/// The whole shared state as one v1 update.
	#[frb(sync)]
	pub fn encode_state(&self) -> Result<Vec<u8>> {
		Ok(self.lock().encode_state()?)
	}

	/// The v1 state vector.
	#[frb(sync)]
	pub fn state_vector(&self) -> Result<Vec<u8>> {
		Ok(self.lock().state_vector()?)
	}

	/// What a peer at `state_vector` is missing, as one v1 update.
	#[frb(sync)]
	pub fn encode_diff(&self, state_vector: &[u8]) -> Result<Vec<u8>> {
		Ok(self.lock().encode_diff(state_vector)?)
	}

	/// Drain this peer's own v1 updates.
	#[frb(sync)]
	pub fn take_updates(&self) -> Result<Vec<Vec<u8>>> {
		Ok(self.lock().take_updates()?)
	}

	/// Check the shared state against the schema.
	#[frb(sync)]
	pub fn validate(&self) -> Result<DocValidation> {
		let validation = self.lock().validate()?;

		Ok(DocValidation {
			rejected_blocks: validation.rejected_blocks,
			waiting: validation.waiting,
		})
	}

	/// The y-sync message to greet a connecting client with.
	#[frb(sync)]
	pub fn sync_start(&self) -> Result<Vec<u8>> {
		Ok(self.lock().sync_start()?)
	}

	/// Handle one y-sync message from a connection.
	#[frb(sync)]
	pub fn receive_sync(&self, message: &[u8]) -> Result<DocSyncReply> {
		let sync = self.lock().receive_sync(message)?;

		Ok(DocSyncReply {
			reply: sync.reply,
			broadcast: sync.broadcast,
			changed: sync.changed,
		})
	}

	/// This peer's changes since the last call, as y-sync messages.
	#[frb(sync)]
	pub fn flush_sync(&self) -> Result<Vec<Vec<u8>>> {
		Ok(self.lock().flush_sync()?)
	}
}
