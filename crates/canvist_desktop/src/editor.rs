//! The editor: a document, its layout, and the viewport onto it.

use std::sync::Arc;
use std::time::Duration;

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::canvist_core::Command;
use canvist_headless::canvist_core::CommandResult;
use canvist_headless::canvist_core::Editor;
use canvist_headless::canvist_core::EditorState;
use canvist_headless::canvist_core::Env;
use canvist_headless::canvist_core::Update;
use canvist_headless::canvist_core::command;
use canvist_headless::canvist_core::commands;
use canvist_headless::canvist_extensions::StarterKitHandles;
use canvist_headless::canvist_model::Commit;
use canvist_headless::canvist_model::Selection;
use canvist_layout::Caret;
use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::LayoutOptions;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Rect;

use crate::DesktopError;
use crate::DesktopResult;
use crate::EditorOptions;
use crate::Fonts;
use crate::accessibility::Accessibility;
use crate::lines::HostLines;
use crate::pointer::PointerState;

/// How far inside the viewport's edges the caret is kept when it is
/// scrolled into view, in logical pixels.
const REVEAL_MARGIN: f32 = 16.0;
/// How many times revealing the caret re-measures: each pass replaces
/// estimated heights above it with measured ones, which can move it again.
const REVEAL_PASSES: usize = 4;

/// The size of the editor on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
	/// The width in logical pixels.
	pub width: f32,
	/// The height in logical pixels.
	pub height: f32,
	/// Physical pixels per logical pixel.
	pub scale: f32,
}

impl Default for Viewport {
	fn default() -> Self {
		Self {
			width: 800.0,
			height: 600.0,
			scale: 1.0,
		}
	}
}

/// What [`DesktopEditor::frame`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameInfo {
	/// The document's height in logical pixels, including estimates for
	/// blocks not laid out yet.
	pub height: f32,
	/// Blocks laid out this frame.
	pub laid_out: usize,
	/// Measurer calls this frame: 0 when every segment was cached.
	pub measured: u64,
	/// Display items to paint.
	pub items: usize,
	/// When the caret next blinks, if it is blinking: the host should draw
	/// a frame then.
	pub next_blink: Option<Duration>,
}

/// The canvist editor for desktop apps.
///
/// It owns a [`Doc`] from an [`Engine`], lays it out with `canvist_layout`
/// and parley, and paints it through any `canvist_paint` painter. Input
/// arrives as [`InputEvent`](crate::InputEvent)s, so it runs headless in
/// tests exactly as it runs in a window; the `winit` feature adds the
/// window host.
///
/// `Handles` are the engine's typed handles, so commands of extensions that
/// were not registered do not compile:
///
/// ```
/// use canvist_desktop::DesktopEditor;
/// use canvist_desktop::EditorOptions;
/// use canvist_desktop::FontSource;
/// use canvist_headless::Engine;
/// use canvist_headless::Format;
///
/// let mut editor = DesktopEditor::new(
/// 	Engine::starter_kit(),
/// 	EditorOptions::new().fonts(FontSource::Bundled),
/// )?;
/// let kit = editor.handles();
///
/// editor.load(Format::Markdown, "Hello")?;
/// editor.select_all()?;
/// editor.run(&kit.bold.toggle())?;
///
/// assert_eq!(editor.export(Format::Markdown), "**Hello**");
/// # Ok::<(), canvist_desktop::DesktopError>(())
/// ```
pub struct DesktopEditor<Handles = StarterKitHandles> {
	pub(crate) engine: Engine<Handles>,
	pub(crate) doc: Doc,
	pub(crate) options: EditorOptions,
	pub(crate) fonts: Fonts,
	pub(crate) measurer: ParleyMeasurer,
	pub(crate) layout: DocumentLayout,
	/// The document version the layout last saw.
	synced: u64,
	pub(crate) viewport: Viewport,
	/// The document coordinate at the top of the viewport, before padding.
	pub(crate) scroll: f32,
	/// What the last frame painted.
	pub(crate) list: DisplayList,
	/// Whether the next frame should scroll the caret into view.
	reveal: bool,
	/// Whether something changed since the last frame.
	pub(crate) dirty: bool,
	pub(crate) focused: bool,
	/// When the caret last restarted its blink: on focus and on every edit.
	pub(crate) blink_from: Duration,
	/// The time of the last input, for the caret blink.
	pub(crate) now: Duration,
	pub(crate) pointer: PointerState,
	pub(crate) a11y: Accessibility,
	#[cfg(feature = "collab")]
	pub(crate) joining: crate::collab::Joining,
}

impl<Handles> std::fmt::Debug for DesktopEditor<Handles> {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("DesktopEditor")
			.field("doc", &self.doc)
			.field("viewport", &self.viewport)
			.field("scroll", &self.scroll)
			.finish_non_exhaustive()
	}
}

impl<Handles: Copy> DesktopEditor<Handles> {
	/// An editor with an empty document from `engine`.
	pub fn new(engine: Engine<Handles>, options: EditorOptions) -> DesktopResult<Self> {
		let engine = engine
			.with_client(options.client)
			.with_platform(options.platform);
		let doc = engine.create()?;
		Ok(Self::with_doc(engine, doc, options))
	}

	fn with_doc(engine: Engine<Handles>, doc: Doc, options: EditorOptions) -> Self {
		let fonts = Fonts::new(options.fonts);
		let viewport = Viewport::default();
		let layout = DocumentLayout::new(
			doc.document(),
			options.theme.clone(),
			layout_options(&options, viewport),
		);

		Self {
			synced: doc.document().version(),
			measurer: fonts.measurer(),
			fonts,
			engine,
			doc,
			options,
			layout,
			viewport,
			scroll: 0.0,
			list: DisplayList::default(),
			reveal: false,
			dirty: true,
			focused: false,
			blink_from: Duration::ZERO,
			now: Duration::ZERO,
			pointer: PointerState::default(),
			a11y: Accessibility::default(),
			#[cfg(feature = "collab")]
			joining: crate::collab::Joining::default(),
		}
	}

	/// The engine's typed handles, for commands and queries.
	#[must_use]
	pub fn handles(&self) -> Handles {
		self.engine.handles()
	}

	/// The engine the document belongs to.
	#[must_use]
	pub fn engine(&self) -> &Engine<Handles> {
		&self.engine
	}

	/// The document.
	#[must_use]
	pub fn doc(&self) -> &Doc {
		&self.doc
	}

	/// The editor state: document, selection, and stored marks, for typed
	/// queries such as `kit.bold.is_active(editor.state())`.
	#[must_use]
	pub fn state(&self) -> &EditorState {
		self.doc.state()
	}

	/// The options the editor was created with.
	#[must_use]
	pub fn options(&self) -> &EditorOptions {
		&self.options
	}

	/// The fonts, to build a painter's shaper.
	#[must_use]
	pub fn fonts(&self) -> &Fonts {
		&self.fonts
	}

	/// The layout, for geometry queries such as `caret_rect`.
	#[must_use]
	pub fn layout(&self) -> &DocumentLayout {
		&self.layout
	}

	/// Replace the document with `input` read as `format`. The caret goes
	/// to the start and the view to the top.
	pub fn load(&mut self, format: Format, input: &str) -> DesktopResult<()> {
		let doc = self.engine.parse(format, input)?;
		self.set_doc(doc)
	}

	/// Replace the document with `doc`, which must come from this editor's
	/// [`DesktopEditor::engine`], such as a shared document joined from Yjs
	/// state.
	pub fn set_doc(&mut self, doc: Doc) -> DesktopResult<()> {
		if !Arc::ptr_eq(doc.document().schema(), self.engine.schema()) {
			return Err(DesktopError::ForeignDocument);
		}

		self.layout = DocumentLayout::new(
			doc.document(),
			self.options.theme.clone(),
			layout_options(&self.options, self.viewport),
		);
		self.synced = doc.document().version();
		self.doc = doc;
		self.scroll = 0.0;
		self.pointer = PointerState::default();
		self.dirty = true;
		Ok(())
	}

	/// The document as `format`.
	#[must_use]
	pub fn export(&self, format: Format) -> String {
		self.doc.export(format)
	}

	/// Run `command`, from the engine's typed handles. Returns whether it
	/// applied.
	pub fn run(&mut self, command: &Command) -> DesktopResult<bool> {
		self.edit(|editor, env| editor.run(command, env))
	}

	/// Whether `command` would apply. Nothing changes.
	pub fn can(&mut self, command: &Command) -> DesktopResult<bool> {
		Ok(self.doc.can(command)?)
	}

	/// Undo the last change.
	pub fn undo(&mut self) -> DesktopResult<bool> {
		self.edit(Editor::undo)
	}

	/// Redo the last undone change.
	pub fn redo(&mut self) -> DesktopResult<bool> {
		self.edit(Editor::redo)
	}

	/// Select the whole document.
	pub fn select_all(&mut self) -> DesktopResult<bool> {
		let select_all = command(commands::select_all);
		self.run(&select_all)
	}

	/// Select `selection`.
	pub fn select(&mut self, selection: Selection) -> DesktopResult<bool> {
		self.edit(|editor, env| editor.select(selection, env))
	}

	/// Register a TrueType or OpenType font from its bytes, for themes that
	/// name it, and return the family names it added. Text laid out with a
	/// fallback face lays out again.
	pub fn register_font(&mut self, data: Vec<u8>) -> Vec<String> {
		let data: Arc<[u8]> = data.into();
		let families = self.measurer.register_font(data.to_vec());
		self.fonts.add(data);
		self.layout.forget_measurements();
		self.dirty = true;
		families
	}

	/// Size the editor: `width` and `height` in logical pixels, at `scale`
	/// physical pixels per logical pixel.
	pub fn resize(&mut self, viewport: Viewport) {
		if viewport == self.viewport {
			return;
		}

		self.viewport = viewport;
		let width = column_width(&self.options, viewport);
		self.layout.set_width(self.doc.document(), width);
		self.dirty = true;
	}

	/// The editor's size.
	#[must_use]
	pub fn viewport(&self) -> Viewport {
		self.viewport
	}

	/// The document coordinate at the top of the viewport.
	#[must_use]
	pub fn scroll_top(&self) -> f32 {
		self.scroll
	}

	/// Scroll so document coordinate `top` is at the top of the viewport,
	/// within the document's bounds.
	pub fn scroll_to(&mut self, top: f32) {
		let top = top.clamp(0.0, self.max_scroll());

		if top.to_bits() != self.scroll.to_bits() {
			self.scroll = top;
			self.dirty = true;
		}
	}

	/// Whether something changed that the next frame should paint.
	#[must_use]
	pub fn needs_repaint(&self) -> bool {
		self.dirty
	}

	/// Lay out the viewport, scroll the caret into view after edits, and
	/// build the display list that [`DesktopEditor::paint`] draws.
	pub fn frame(&mut self, now: Duration) -> DesktopResult<FrameInfo> {
		self.now = self.now.max(now);
		let mut info = self.ensure_viewport()?;

		if std::mem::take(&mut self.reveal) {
			self.reveal_caret(&mut info)?;
		}

		self.scroll = self.scroll.clamp(0.0, self.max_scroll());
		let (top, bottom) = self.visible_range();
		self.list = self.layout.display_list(&self.display_options(top, bottom));
		// Layout culls whole blocks; drop the lines of long blocks that are
		// off screen too, so painting costs what the screen shows.
		self.list.items.retain(|item| {
			let bounds = item.bounds();
			bounds.y < bottom && bounds.y + bounds.height > top
		});
		self.dirty = false;

		info.height = self.layout.height();
		info.items = self.list.items.len();
		info.next_blink = self.next_blink();
		Ok(info)
	}

	/// The display list the last frame built, in document coordinates.
	#[must_use]
	pub fn display_list(&self) -> &DisplayList {
		&self.list
	}

	/// The caret, when the selection is collapsed.
	#[must_use]
	pub fn caret(&self) -> Option<Caret> {
		match self.doc.selection() {
			Selection::Text { anchor, head } if anchor == head => {
				let affinity = self.doc.state().affinity().into();
				Some(Caret::with_affinity(head, affinity))
			}
			_ => None,
		}
	}

	/// The caret's rectangle, or the selection head's, in the editor's
	/// logical coordinates: where IME candidate windows go.
	#[must_use]
	pub fn caret_area(&self) -> Option<Rect> {
		let head = match self.doc.selection() {
			Selection::Text { head, .. } => Caret::new(head),
			Selection::Blocks { .. } => return None,
		};

		let caret = self.caret().unwrap_or(head);
		self.layout
			.caret_rect(caret)
			.map(|rect| self.to_view_rect(rect))
	}

	/// Make a change through the editor with this layout's line navigator,
	/// then let layout account for it.
	pub(crate) fn edit(
		&mut self,
		change: impl FnOnce(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> DesktopResult<bool> {
		Ok(self
			.edit_update(change)?
			.is_some_and(|update| update.doc_changed() || update.selection_changed()))
	}

	/// [`DesktopEditor::edit`], returning the update it applied.
	pub(crate) fn edit_update(
		&mut self,
		change: impl FnOnce(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> DesktopResult<Option<Update>> {
		let mut lines = HostLines::new(&mut self.layout, &mut self.measurer);
		let update = self.doc.edit(self.now, &mut lines, change)?;

		if let Some(update) = &update {
			self.track(std::slice::from_ref(&update.commit));
			self.reveal = true;
			self.blink_from = self.now;
		}

		Ok(update)
	}

	/// Tell layout about `commits`, applied to the document in order. When
	/// the document moved further than they account for (a collaboration
	/// binding can apply a follow-up of its own), layout re-reads it.
	pub(crate) fn track(&mut self, commits: &[Commit]) {
		let doc = self.doc.document();

		for commit in commits {
			self.layout.apply(doc, commit);
		}

		if doc.version() != commits.last().map_or(self.synced, |commit| commit.version) {
			self.layout.resync(doc);
		}

		self.synced = doc.version();
		self.dirty = true;
	}

	/// The display options for document range `top..bottom`: the caret while
	/// focused and in the visible half of its blink, or the selection.
	fn display_options(&self, top: f32, bottom: f32) -> DisplayOptions {
		let options = DisplayOptions::viewport(top, bottom);

		let Some(caret) = self.caret() else {
			return options.selection(self.doc.selection());
		};

		if self.focused && self.caret_on() {
			options.caret(caret)
		} else {
			options
		}
	}

	/// Whether the blinking caret is in its visible half.
	pub(crate) fn caret_on(&self) -> bool {
		let period = self.options.caret_blink;

		if period.is_zero() {
			return true;
		}

		let elapsed = self.now.saturating_sub(self.blink_from);
		(elapsed.as_nanos() / period.as_nanos()).is_multiple_of(2)
	}

	/// When the caret next turns on or off, on the clock input events are
	/// stamped with, or `None` while it is not blinking.
	#[must_use]
	pub fn next_blink(&self) -> Option<Duration> {
		let period = self.options.caret_blink;

		if !self.focused || period.is_zero() || self.caret().is_none() {
			return None;
		}

		let elapsed = self.now.saturating_sub(self.blink_from).as_nanos();
		let phases = elapsed / period.as_nanos() + 1;
		let next = u64::try_from(phases * period.as_nanos()).unwrap_or(u64::MAX);
		Some(self.blink_from + Duration::from_nanos(next))
	}

	/// Lay out the visible range, keeping content on screen still when
	/// blocks above it replaced estimated heights with measured ones.
	fn ensure_viewport(&mut self) -> DesktopResult<FrameInfo> {
		let (top, bottom) = self.visible_range();
		let update = self
			.layout
			.ensure(self.doc.document(), &mut self.measurer, top, bottom)?;
		self.scroll += update.shift;

		Ok(FrameInfo {
			laid_out: update.laid_out,
			measured: update.measurer_calls,
			..FrameInfo::default()
		})
	}

	/// Scroll just far enough that the caret (or the selection's head) is
	/// inside the viewport, measuring the blocks on the way.
	fn reveal_caret(&mut self, info: &mut FrameInfo) -> DesktopResult<()> {
		for _ in 0..REVEAL_PASSES {
			let Some(target) = self.head_rect().and_then(|rect| self.reveal_target(rect)) else {
				return Ok(());
			};

			self.scroll = target.clamp(0.0, self.max_scroll());
			let pass = self.ensure_viewport()?;
			info.laid_out += pass.laid_out;
			info.measured += pass.measured;
		}

		Ok(())
	}

	/// Where to scroll so `rect`, in document coordinates, is inside the
	/// viewport with a margin, or `None` when it already is.
	fn reveal_target(&self, rect: Rect) -> Option<f32> {
		let padding = self.options.padding;
		let top = rect.y + padding - REVEAL_MARGIN;
		let bottom = rect.y + rect.height + padding + REVEAL_MARGIN;

		if top < self.scroll {
			return Some(top);
		}

		(bottom > self.scroll + self.viewport.height).then_some(bottom - self.viewport.height)
	}

	/// The rectangle of the selection's head, from layout when its block is
	/// laid out and from the block's estimated rectangle otherwise.
	fn head_rect(&self) -> Option<Rect> {
		let Selection::Text { head, .. } = self.doc.selection() else {
			return None;
		};
		let caret = self.caret().unwrap_or(Caret::new(head));

		self.layout
			.caret_rect(caret)
			.or_else(|| self.layout.block_rect(head.block))
	}

	/// The document range the viewport shows.
	pub(crate) fn visible_range(&self) -> (f32, f32) {
		let top = self.scroll - self.options.padding;
		(top, top + self.viewport.height)
	}

	fn max_scroll(&self) -> f32 {
		let content = self.layout.height() + self.options.padding * 2.0;
		(content - self.viewport.height).max(0.0)
	}

	/// Where the document's left edge is in the editor.
	pub(crate) fn column_left(&self) -> f32 {
		let width = column_width(&self.options, self.viewport);
		let spare = self.viewport.width - self.options.padding * 2.0 - width;
		self.options.padding + (spare / 2.0).max(0.0)
	}

	/// A document point in the editor's logical coordinates.
	pub(crate) fn to_view(&self, x: f32, y: f32) -> (f32, f32) {
		(
			x + self.column_left(),
			y - self.scroll + self.options.padding,
		)
	}

	/// An editor point in document coordinates.
	pub(crate) fn to_document(&self, x: f32, y: f32) -> (f32, f32) {
		(
			x - self.column_left(),
			y + self.scroll - self.options.padding,
		)
	}

	pub(crate) fn to_view_rect(&self, rect: Rect) -> Rect {
		let (x, y) = self.to_view(rect.x, rect.y);
		Rect::new(x, y, rect.width, rect.height)
	}
}

/// The document column's width in `viewport`.
fn column_width(options: &EditorOptions, viewport: Viewport) -> f32 {
	let available = viewport.width - options.padding * 2.0;
	options
		.max_width
		.map_or(available, |max| available.min(max))
		.max(1.0)
}

fn layout_options(options: &EditorOptions, viewport: Viewport) -> LayoutOptions {
	LayoutOptions::new(column_width(options, viewport)).overscan(options.overscan)
}
