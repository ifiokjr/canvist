//! The engine: a set of extensions, configured once and shared.

use std::sync::Arc;
use std::sync::OnceLock;

use canvist_editor::Editor;
use canvist_editor::EditorBuilder;
use canvist_model::Document;
use canvist_model::Schema;
use serde_json::Value as Json;

use crate::Doc;
use crate::Format;
use crate::HeadlessError;
use crate::Snapshot;
use crate::format;

/// Registers the extensions on a builder and returns their typed handles.
type Configure<H> = dyn Fn(&mut EditorBuilder) -> H + Send + Sync;

/// A configured set of extensions: their schema, their metadata, and their
/// typed handles. It creates [`Doc`]s.
///
/// Build one per process and share it: an engine is cheap to clone, and it
/// is `Send` and `Sync`, so request handlers on any thread can create
/// documents from it.
///
/// ```
/// use canvist_headless::Engine;
/// use canvist_headless::Format;
///
/// let engine = Engine::starter_kit();
/// let doc = engine.parse(Format::Markdown, "# Notes\n\nShip **it**.")?;
///
/// assert_eq!(doc.export(Format::Html), "<h1>Notes</h1><p>Ship <strong>it</strong>.</p>");
/// # Ok::<(), canvist_headless::HeadlessError>(())
/// ```
pub struct Engine<H = ()> {
	configure: Arc<Configure<H>>,
	handles: H,
	schema: Arc<Schema>,
	/// Built on first use: most hosts never ask for it.
	metadata: Arc<OnceLock<Json>>,
	client: u32,
}

impl<H: Copy> Clone for Engine<H> {
	fn clone(&self) -> Self {
		Self {
			configure: self.configure.clone(),
			handles: self.handles,
			schema: self.schema.clone(),
			metadata: self.metadata.clone(),
			client: self.client,
		}
	}
}

impl<H> std::fmt::Debug for Engine<H> {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Engine")
			.field("client", &self.client)
			.finish_non_exhaustive()
	}
}

#[cfg(feature = "starter-kit")]
impl Engine<canvist_extensions::StarterKitHandles> {
	/// An engine with TipTap's StarterKit: paragraphs, headings, lists, task
	/// lists, quotes, code blocks, rules, hard breaks, the five basic marks,
	/// links, and history. Its handles reach every StarterKit command.
	///
	/// # Panics
	///
	/// Never: the StarterKit is a fixed, tested configuration.
	#[must_use]
	pub fn starter_kit() -> Self {
		Self::new(canvist_extensions::StarterKit::add_all)
			.expect("the StarterKit is a valid configuration")
	}
}

impl<H: Copy> Engine<H> {
	/// An engine whose documents use the extensions `configure` registers.
	/// `configure` returns the typed handles, which [`Engine::handles`] hands
	/// back, so commands of extensions that were not registered cannot be
	/// named:
	///
	/// ```
	/// use canvist_extensions::Bold;
	/// use canvist_extensions::Document;
	/// use canvist_extensions::Paragraph;
	/// use canvist_headless::Engine;
	/// use canvist_headless::Format;
	///
	/// let engine = Engine::new(|builder| {
	/// 	builder.add(Document);
	/// 	builder.add(Paragraph);
	/// 	builder.add(Bold)
	/// })?;
	/// let bold = engine.handles();
	///
	/// let mut doc = engine.parse(Format::Text, "Hello")?;
	/// doc.select_all()?;
	/// doc.run(&bold.toggle())?;
	///
	/// assert_eq!(doc.export(Format::Markdown), "**Hello**");
	/// # Ok::<(), canvist_headless::HeadlessError>(())
	/// ```
	///
	/// `configure` runs again for every document, because each document owns
	/// its own editor. It must register the same extensions every time.
	pub fn new(
		configure: impl Fn(&mut EditorBuilder) -> H + Send + Sync + 'static,
	) -> Result<Self, HeadlessError> {
		let mut builder = Editor::builder();
		let handles = configure(&mut builder);
		let editor = builder.build().map_err(HeadlessError::Config)?;

		Ok(Self {
			configure: Arc::new(configure),
			handles,
			schema: editor.schema().clone(),
			metadata: Arc::new(OnceLock::new()),
			client: 1,
		})
	}

	/// Allocate block ids for new blocks as `client`, which collaborating
	/// peers must not share. The default is 1.
	#[must_use]
	pub fn with_client(mut self, client: u32) -> Self {
		self.client = client;
		self
	}

	/// The typed handles `configure` returned.
	#[must_use]
	pub fn handles(&self) -> H {
		self.handles
	}

	/// The schema the extensions describe.
	#[must_use]
	pub fn schema(&self) -> &Arc<Schema> {
		&self.schema
	}

	/// What every extension adds (blocks, marks, embeds, commands, queries,
	/// and their argument types) as JSON: the input binding generators read.
	#[must_use]
	pub fn metadata(&self) -> &Json {
		self.metadata.get_or_init(|| {
			let mut builder = Editor::builder();
			(self.configure)(&mut builder);
			builder
				.build()
				.expect("`Engine::new` built the same extensions without error")
				.metadata_json()
		})
	}

	/// The client id new blocks are allocated under.
	#[must_use]
	pub fn client(&self) -> u32 {
		self.client
	}

	/// A document with one empty paragraph.
	pub fn create(&self) -> Result<Doc, HeadlessError> {
		self.wrap(Document::new(self.schema.clone(), self.client))
	}

	/// Read `input` as `format`. JSON must fit the schema; HTML, Markdown,
	/// and text fall back to paragraphs and plain text for anything the
	/// schema has no type for, so they never fail on content.
	pub fn parse(&self, format: Format, input: &str) -> Result<Doc, HeadlessError> {
		self.wrap(format::read(&self.schema, self.client, format, input)?)
	}

	/// Read ProseMirror or TipTap JSON that is already parsed.
	pub fn from_json(&self, json: &Json) -> Result<Doc, HeadlessError> {
		self.wrap(Document::from_json(self.schema.clone(), self.client, json)?)
	}

	/// Recreate the document a [`Snapshot`] captured, with its block ids and
	/// selection.
	pub fn restore(&self, snapshot: &Snapshot) -> Result<Doc, HeadlessError> {
		let mut doc = self.from_json(snapshot.content())?;

		if let Some((anchor, head)) = snapshot.selection() {
			doc.select_range(anchor, head)?;
		}

		Ok(doc)
	}

	/// Wrap a model document created with this engine's schema.
	pub(crate) fn wrap(&self, doc: Document) -> Result<Doc, HeadlessError> {
		Ok(Doc::new(self.editor(doc)?))
	}

	/// An editor for `doc`, configured with this engine's extensions. `doc`
	/// must use this engine's schema.
	pub(crate) fn editor(&self, doc: Document) -> Result<Editor, HeadlessError> {
		debug_assert!(Arc::ptr_eq(doc.schema(), &self.schema));
		let mut builder = Editor::builder().client(self.client);
		(self.configure)(&mut builder);
		builder
			.build_with(|_, _| Ok(doc))
			.map_err(HeadlessError::Config)
	}
}
