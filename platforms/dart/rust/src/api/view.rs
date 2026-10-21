//! An interactive view of one document: the layout, the display list a host
//! paints, and the input a host forwards, for UI toolkits such as Flutter.
//!
//! The view shares its document with the [`CanvistDoc`] that created it.
//! The document records every commit (see
//! [`Doc::record_commits`](canvist_headless::Doc::record_commits)), so
//! changes made through the document directly, such as commands run on it
//! or collaborators' updates, reach the layout too: the view brings its
//! layout up to date before every call.

use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::time::Duration;
use std::time::SystemTime;

use canvist_headless::Doc;
use canvist_headless::HeadlessError;
use canvist_headless::canvist_editor::CommandResult;
use canvist_headless::canvist_editor::Editor;
use canvist_headless::canvist_editor::Env;
use canvist_headless::canvist_editor::KeyPress;
use canvist_headless::canvist_editor::Modifiers;
use canvist_headless::canvist_editor::Platform;
use canvist_headless::canvist_editor::Slice;
use canvist_headless::canvist_editor::command;
use canvist_headless::canvist_editor::commands;
use canvist_headless::canvist_editor::position;
use canvist_headless::canvist_extensions::clipboard;
use canvist_headless::canvist_model::BlockId;
use canvist_headless::canvist_model::Document;
use canvist_headless::canvist_model::InlineText;
use canvist_headless::canvist_model::Point;
use canvist_headless::canvist_model::Selection;
use canvist_layout::Caret;
use canvist_layout::Color;
use canvist_layout::ColorScheme;
use canvist_layout::DecorationKind;
use canvist_layout::DisplayItem;
use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::LayoutError;
use canvist_layout::LayoutOptions;
use canvist_layout::Rect;
use canvist_layout::RectKind;
use canvist_layout::SlotKind;
use canvist_layout::StarterKitOptions;
use canvist_layout::Theme;
use flutter_rust_bridge::frb;
use serde_json::Value as Json;
use unicode_segmentation::UnicodeSegmentation;

use crate::api::canvist::CanvistFailure;
use crate::lines::HostLines;
use crate::measure::HostMeasurer;

type Result<T> = std::result::Result<T, CanvistFailure>;

/// Clicks that select a word; more select the whole text block.
const WORD_CLICKS: u32 = 2;

/// How the view looks: the StarterKit theme's options.
#[derive(Clone, Debug)]
pub struct ViewOptions {
	/// Dark colours instead of light ones.
	pub dark: bool,
	/// The font family list for body text and headings, as CSS writes it.
	pub body_family: Option<String>,
	/// The font family list for code.
	pub code_family: Option<String>,
	/// The body font size in logical pixels.
	pub size: Option<f32>,
	/// The body line height, as a multiple of the font size.
	pub line_height: Option<f32>,
	/// How far beyond the viewport to lay out, in logical pixels.
	pub overscan: f32,
}

impl ViewOptions {
	fn theme(&self) -> Theme {
		let mut starter = StarterKitOptions {
			scheme: if self.dark {
				ColorScheme::Dark
			} else {
				ColorScheme::Light
			},
			..StarterKitOptions::default()
		};

		if let Some(family) = &self.body_family {
			starter.body_family = Arc::from(family.as_str());
		}

		if let Some(family) = &self.code_family {
			starter.code_family = Arc::from(family.as_str());
		}

		if let Some(size) = self.size.filter(|size| *size > 0.0) {
			starter.size = size;
		}

		if let Some(line_height) = self.line_height.filter(|height| *height > 0.0) {
			starter.line_height = line_height;
		}

		Theme::starter_kit_with(&starter)
	}
}

/// A font a display list draws with.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFont {
	/// The CSS `font-family` list.
	pub family: String,
	/// The size in logical pixels.
	pub size: f32,
	/// The CSS weight, 100 to 900.
	pub weight: u16,
	/// Whether the face is italic.
	pub italic: bool,
}

/// What a [`ViewItem`] draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewItemKind {
	/// A block or container background.
	Background,
	/// A container's border, such as a blockquote bar.
	Border,
	/// A horizontal rule.
	Rule,
	/// An inline background, such as inline code or a highlight mark.
	Highlight,
	/// The selection.
	Selection,
	/// The caret.
	Caret,
	/// A run of text.
	Text,
	/// An underline, drawn as a filled rectangle.
	Underline,
	/// A strikethrough, drawn as a filled rectangle.
	Strike,
	/// A wavy line from `x` to `x + width` along `y`, `height` thick.
	Squiggle,
	/// An ink stroke: a polyline through [`ViewItem::points`],
	/// [`ViewItem::line_width`] wide, with round caps and joins. Its
	/// rectangle bounds the points and caps.
	Stroke,
	/// A task item's checkbox.
	Checkbox,
	/// Space reserved for a view the host renders, such as an image.
	Slot,
	/// The IME composition's underline.
	Composition,
}

/// One thing to paint, in document coordinates.
///
/// Rectangles use `x`, `y`, `width`, and `height`. Text draws `text` in
/// font `font` with its baseline at `baseline`, starting at `x`, and its
/// line box in the rectangle. A squiggle runs along `y`, `height` thick.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewItem {
	/// What to draw.
	pub kind: ViewItemKind,
	/// The left edge.
	pub x: f32,
	/// The top edge, or a squiggle's centre line.
	pub y: f32,
	/// The width.
	pub width: f32,
	/// The height, or a squiggle's thickness.
	pub height: f32,
	/// The fill as `0xAARRGGBB`.
	pub color: u32,
	/// A rectangle's corner radius.
	pub radius: f32,
	/// The text of a text run.
	pub text: String,
	/// A text run's font, indexed into [`ViewDisplay::fonts`].
	pub font: u32,
	/// A text run's baseline.
	pub baseline: f32,
	/// Extra space after every grapheme of a text run.
	pub letter_spacing: f32,
	/// Whether a text run is right to left.
	pub rtl: bool,
	/// Whether a checkbox is checked.
	pub checked: bool,
	/// A stroke's points, as `x, y` pairs.
	pub points: Vec<f32>,
	/// A stroke's line width.
	pub line_width: f32,
}

impl ViewItem {
	fn rect(kind: ViewItemKind, rect: Rect, color: Color) -> Self {
		Self {
			kind,
			x: rect.x,
			y: rect.y,
			width: rect.width,
			height: rect.height,
			color: argb(color),
			radius: 0.0,
			text: String::new(),
			font: 0,
			baseline: 0.0,
			letter_spacing: 0.0,
			rtl: false,
			checked: false,
			points: Vec::new(),
			line_width: 0.0,
		}
	}
}

/// Everything to paint for the viewport.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewDisplay {
	/// Fonts, indexed by [`ViewItem::font`]. Ids are stable for the view's
	/// life, so painters can cache per id.
	pub fonts: Vec<ViewFont>,
	/// Items in paint order.
	pub items: Vec<ViewItem>,
	/// The background as `0xAARRGGBB`.
	pub background: u32,
}

/// What one [`CanvistView::frame`] produced.
#[derive(Clone, Debug)]
pub struct ViewFrame {
	/// The document's height, including estimates for blocks not laid out
	/// yet. Hosts size their scroll extent to it.
	pub height: f32,
	/// How far content at the top of the viewport moved because blocks
	/// above it were measured. Add it to the scroll offset to keep the
	/// content still.
	pub shift: f32,
	/// Blocks laid out for this frame.
	pub laid_out: u32,
	/// Measurer calls for this frame: 0 when every segment was cached.
	pub measured: u32,
	/// What to paint, or `None` when it is what the last frame returned.
	pub display: Option<ViewDisplay>,
}

/// A rectangle in document coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewRect {
	/// The left edge.
	pub x: f32,
	/// The top edge.
	pub y: f32,
	/// The width.
	pub width: f32,
	/// The height.
	pub height: f32,
}

impl From<Rect> for ViewRect {
	fn from(rect: Rect) -> Self {
		Self {
			x: rect.x,
			y: rect.y,
			width: rect.width,
			height: rect.height,
		}
	}
}

/// Where a text selection's ends are drawn, for selection handles.
#[derive(Clone, Copy, Debug)]
pub struct ViewSelectionEnds {
	/// The caret rectangle at the start of the selection.
	pub start: ViewRect,
	/// The caret rectangle at its end.
	pub end: ViewRect,
	/// Whether the selection is a caret.
	pub collapsed: bool,
}

/// The text a platform input method edits: the text block holding the
/// selection's head, with the selection and composition in it, in UTF-16
/// code units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewEditState {
	/// The block's text.
	pub text: String,
	/// The selection's anchor, clamped to the block.
	pub anchor: u32,
	/// The selection's head.
	pub head: u32,
	/// Where the IME composition starts, or `None` without one.
	pub composing_start: Option<u32>,
	/// Where it ends.
	pub composing_end: Option<u32>,
}

/// The document as assistive technology reads it: its plain text, with text
/// blocks on their own lines, and the selection in UTF-16 code units of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewSemantics {
	/// The plain text.
	pub text: String,
	/// Where the selection's anchor is in it.
	pub anchor: u32,
	/// Where its head is.
	pub head: u32,
}

/// The selection as clipboard formats.
#[derive(Clone, Debug)]
pub struct ViewClipboard {
	/// Semantic HTML.
	pub html: String,
	/// Plain text.
	pub text: String,
}

/// What a pointer drag extends the selection by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
	Character,
	Word,
	Block,
}

/// A pointer drag in progress: the range the press selected, which every
/// later position extends from.
#[derive(Clone, Copy, Debug)]
struct Drag {
	unit: Unit,
	from: Point,
	to: Point,
}

impl Drag {
	/// The selection from the press to `point`, extended by the drag's unit.
	fn extended(self, doc: &Doc, point: Point) -> Selection {
		let (start, end) = unit_around(doc, point, self.unit);

		if position::compare_points(doc.document(), point, self.from) == Ordering::Less {
			Selection::text(self.to, start)
		} else {
			Selection::text(self.from, end)
		}
	}
}

struct ViewState {
	layout: DocumentLayout,
	measurer: HostMeasurer,
	theme: Arc<Theme>,
	/// What the last frame returned, to skip unchanged ones.
	painted: Option<ViewDisplay>,
	focused: bool,
	caret_on: bool,
	drag: Option<Drag>,
}

/// A document laid out for a host to paint and edit.
#[frb(opaque)]
pub struct CanvistView {
	doc: Arc<Mutex<Doc>>,
	state: Mutex<ViewState>,
}

impl CanvistView {
	/// A view of `doc` measuring text through the Dart function at
	/// `measurer`.
	pub(crate) fn new(doc: Arc<Mutex<Doc>>, measurer: HostMeasurer, options: &ViewOptions) -> Self {
		let theme = Arc::new(options.theme());
		let layout = {
			let mut doc = lock(&doc);
			doc.record_commits();
			// Earlier changes are already in the document the layout starts
			// from.
			doc.take_commits();
			DocumentLayout::new(
				doc.document(),
				theme.clone(),
				LayoutOptions::new(0.0).overscan(options.overscan),
			)
		};

		Self {
			doc,
			state: Mutex::new(ViewState {
				layout,
				measurer,
				theme,
				painted: None,
				focused: false,
				caret_on: true,
				drag: None,
			}),
		}
	}

	/// Lay the document out `width` logical pixels wide.
	#[frb(sync)]
	pub fn set_width(&self, width: f32) {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);

		if state.layout.options().width > 0.0 {
			state.layout.set_width(doc.document(), width);
			return;
		}

		// A new width keeps every block's height as its estimate until the
		// block is laid out again, and estimates made zero pixels wide put
		// each character on its own line: start over at the first real width.
		let options = LayoutOptions::new(width).overscan(state.layout.options().overscan);
		state.layout = DocumentLayout::new(doc.document(), state.theme.clone(), options);
	}

	/// Change the theme. Measurements are kept.
	#[frb(sync)]
	pub fn set_options(&self, options: ViewOptions) {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let theme = Arc::new(options.theme());
		state.layout.set_theme(doc.document(), theme.clone());
		state.theme = theme;
	}

	/// Forget every measurement, as when fonts load or the text scale
	/// changes: blocks lay out again as they come into view.
	#[frb(sync)]
	pub fn forget_measurements(&self) {
		let mut state = self.state();
		state.layout.forget_measurements();
		state.measurer.resend_fonts();
		state.painted = None;
	}

	/// Show the caret while the host has focus.
	#[frb(sync)]
	pub fn set_focused(&self, focused: bool) {
		let mut state = self.state();
		state.focused = focused;
		state.caret_on = true;
	}

	/// Blink the caret.
	#[frb(sync)]
	pub fn set_caret_on(&self, on: bool) {
		self.state().caret_on = on;
	}

	/// Lay out the viewport from `top` down `height` logical pixels and say
	/// what to paint.
	#[frb(sync)]
	pub fn frame(&self, top: f32, height: f32) -> Result<ViewFrame> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let state = &mut *state;
		let bottom = top + height;
		let update = state
			.layout
			.ensure(doc.document(), &mut state.measurer, top, bottom)
			.map_err(|error| render(&error))?;
		let options = state.display_options(&doc, top, bottom);
		let list = state.layout.display_list(&options);
		let display = state.display(&doc, &list);
		let changed = state.painted.as_ref() != Some(&display);

		if changed {
			state.painted = Some(display.clone());
		}

		Ok(ViewFrame {
			height: state.layout.height(),
			shift: update.shift,
			laid_out: update.laid_out as u32,
			measured: update.measurer_calls as u32,
			display: changed.then_some(display),
		})
	}

	/// Run the key binding for `key`, a DOM `KeyboardEvent.key` name such as
	/// `Enter`, `ArrowLeft`, or `b`. Returns whether a binding applied; when
	/// none did, the host lets the platform handle the key.
	#[frb(sync)]
	pub fn key_down(
		&self,
		key: &str,
		shift: bool,
		ctrl: bool,
		alt: bool,
		meta: bool,
	) -> Result<bool> {
		let modifiers = Modifiers {
			shift,
			ctrl,
			alt,
			meta,
		};

		// The document lock must be released before undoing, which takes it.
		let history = history_key(&lock(&self.doc), key, modifiers);

		match history {
			Some(History::Undo) => self.undo(),
			Some(History::Redo) => self.redo(),
			None => self.edit(|editor, env| editor.handle_key(KeyPress::new(key, modifiers), env)),
		}
	}

	/// Replace the selection with `text` as typing does: input rules apply.
	#[frb(sync)]
	pub fn insert_text(&self, text: &str) -> Result<bool> {
		self.edit(|editor, env| editor.insert_text(text, env))
	}

	/// The text block an input method edits, or `None` when the head is not
	/// in a text block.
	#[frb(sync)]
	pub fn edit_state(&self) -> Option<ViewEditState> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let (block, text) = head_block(&doc)?;
		let (anchor, head) = selection_points(&doc);
		let document = doc.document();
		let clamp = |point: Point| {
			if point.block == block {
				utf16(&text, point.offset)
			} else if position::compare_points(document, point, Point::new(block, 0))
				== Ordering::Less
			{
				0
			} else {
				utf16(&text, text.len() as u32)
			}
		};
		let composition = doc
			.state()
			.composition()
			.filter(|composition| composition.block == block);

		Some(ViewEditState {
			anchor: clamp(anchor),
			head: clamp(head),
			composing_start: composition.map(|composition| utf16(&text, composition.start)),
			composing_end: composition.map(|composition| utf16(&text, composition.end)),
			text,
		})
	}

	/// Select UTF-16 range `anchor..head` of the head's text block.
	#[frb(sync)]
	pub fn select_in_block(&self, anchor: u32, head: u32) -> Result<bool> {
		let Some(selection) = self.block_selection(anchor, head) else {
			return Ok(false);
		};
		self.edit(|editor, env| editor.select(selection, env))
	}

	/// Replace UTF-16 range `start..end` of the head's text block with
	/// `text`, as an input method does. A replacement at a caret is typing,
	/// so input rules apply.
	#[frb(sync)]
	pub fn replace_text(&self, start: u32, end: u32, text: &str) -> Result<bool> {
		let Some(range) = self.block_selection(start, end) else {
			return Ok(false);
		};
		let moved = range != lock(&self.doc).selection()
			&& self.edit(|editor, env| editor.select(range, env))?;

		if text.is_empty() {
			let deleted =
				self.edit(|editor, env| editor.run(&command(commands::delete_selection), env))?;
			return Ok(moved || deleted);
		}

		Ok(self.edit(|editor, env| editor.insert_text(text, env))? || moved)
	}

	/// Show `text` as IME preedit text, replacing the selection or the
	/// previous preedit.
	#[frb(sync)]
	pub fn set_composition(&self, text: &str) -> Result<bool> {
		self.edit(|editor, env| editor.set_composition(text, env))
	}

	/// Commit the composition as `text`.
	#[frb(sync)]
	pub fn commit_composition(&self, text: &str) -> Result<bool> {
		self.edit(|editor, env| editor.commit_composition(text, env))
	}

	/// Remove the preedit and end the composition.
	#[frb(sync)]
	pub fn cancel_composition(&self) -> Result<bool> {
		self.edit(Editor::cancel_composition)
	}

	/// Press the pointer at document coordinates `(x, y)`. One click places
	/// the caret, or extends the selection with `extend`; two select a word
	/// and three the whole text block, and dragging afterwards extends by
	/// that unit.
	#[frb(sync)]
	pub fn pointer_down(&self, x: f32, y: f32, extend: bool, clicks: u32) -> Result<bool> {
		let selection = {
			let (mut state, mut doc) = self.lock();
			state.sync(&mut doc);
			let Some(point) = state.point_at(x, y) else {
				return Ok(false);
			};
			let unit = match clicks {
				0 | 1 => Unit::Character,
				WORD_CLICKS => Unit::Word,
				_ => Unit::Block,
			};

			if extend && unit == Unit::Character {
				let (anchor, _) = selection_points(&doc);
				let drag = Drag {
					unit,
					from: anchor,
					to: anchor,
				};
				state.drag = Some(drag);
				drag.extended(&doc, point)
			} else {
				let (from, to) = unit_around(&doc, point, unit);
				state.drag = Some(Drag { unit, from, to });
				Selection::text(from, to)
			}
		};

		self.edit(|editor, env| editor.select(selection, env))
	}

	/// Move a pressed pointer to `(x, y)`, extending the selection from the
	/// press.
	#[frb(sync)]
	pub fn pointer_move(&self, x: f32, y: f32) -> Result<bool> {
		let selection = {
			let (mut state, mut doc) = self.lock();
			state.sync(&mut doc);

			match (state.drag, state.point_at(x, y)) {
				(Some(drag), Some(point)) => drag.extended(&doc, point),
				_ => return Ok(false),
			}
		};
		self.edit(|editor, env| editor.select(selection, env))
	}

	/// Release the pointer.
	#[frb(sync)]
	pub fn pointer_up(&self) {
		self.state().drag = None;
	}

	/// Whether `(x, y)` is inside a non-empty selection.
	#[frb(sync)]
	pub fn is_in_selection(&self, x: f32, y: f32) -> bool {
		let (state, doc) = self.lock();
		let document = doc.document();
		let Some(range) = position::text_range(document, doc.selection()) else {
			return false;
		};
		let Some(point) = state.point_at(x, y) else {
			return false;
		};

		!range.is_empty()
			&& position::compare_points(document, range.from, point) == Ordering::Less
			&& position::compare_points(document, point, range.to) == Ordering::Less
	}

	/// Move one end of the selection to `(x, y)`, keeping the other: the
	/// start when `start` is true, otherwise the end. Selection handles drag
	/// this way.
	#[frb(sync)]
	pub fn move_selection_end(&self, start: bool, x: f32, y: f32) -> Result<bool> {
		let selection = {
			let (mut state, mut doc) = self.lock();
			state.sync(&mut doc);
			let Some(point) = state.point_at(x, y) else {
				return Ok(false);
			};
			let Some(range) = position::text_range(doc.document(), doc.selection()) else {
				return Ok(false);
			};

			if start {
				Selection::text(range.to, point)
			} else {
				Selection::text(range.from, point)
			}
		};
		self.edit(|editor, env| editor.select(selection, env))
	}

	/// The selection as HTML and plain text, or `None` when nothing is
	/// selected.
	#[frb(sync)]
	pub fn copy_selection(&self) -> Result<Option<ViewClipboard>> {
		let doc = lock(&self.doc);

		if selection_is_collapsed(&doc) {
			return Ok(None);
		}

		let slice = doc
			.editor()
			.copy()
			.map_err(|error| CanvistFailure::from(HeadlessError::from(error)))?;
		Ok(Some(clipboard_formats(doc.document(), &slice)))
	}

	/// Cut the selection, returning it as [`CanvistView::copy_selection`]
	/// does.
	#[frb(sync)]
	pub fn cut_selection(&self) -> Result<Option<ViewClipboard>> {
		let copied = self.copy_selection()?;

		if copied.is_some() {
			self.edit(|editor, env| editor.run(&command(commands::delete_selection), env))?;
		}

		Ok(copied)
	}

	/// Paste plain text over the selection; paste rules apply.
	#[frb(sync)]
	pub fn paste_text(&self, text: &str) -> Result<bool> {
		self.edit(|editor, env| editor.paste_text(text, env))
	}

	/// Paste HTML over the selection.
	#[frb(sync)]
	pub fn paste_html(&self, html: &str) -> Result<bool> {
		let slice = clipboard::from_html(lock(&self.doc).document().schema(), html);
		self.edit(|editor, env| editor.paste(slice, env))
	}

	/// Run the command an extension registered as `name`, with JSON `args`.
	#[frb(sync)]
	pub fn run(&self, name: &str, args: &str) -> Result<bool> {
		// A shared document undoes only this peer's changes, through the
		// shared history, never the editor's own.
		let shared = lock(&self.doc).is_shared();

		if shared {
			match name {
				"undo" => return self.undo(),
				"redo" => return self.redo(),
				_ => {}
			}
		}

		let args = parse_json(args)?;
		let command = lock(&self.doc).command_named(name, &args)?;
		self.edit(|editor, env| editor.run(&command, env))
	}

	/// Undo the last change.
	#[frb(sync)]
	pub fn undo(&self) -> Result<bool> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let undone = doc.undo()?;
		state.sync(&mut doc);
		state.caret_on = true;
		Ok(undone)
	}

	/// Redo the last undone change.
	#[frb(sync)]
	pub fn redo(&self) -> Result<bool> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let redone = doc.redo()?;
		state.sync(&mut doc);
		state.caret_on = true;
		Ok(redone)
	}

	/// The caret's rectangle, or `None` when the selection is not a caret.
	/// Lays out the caret's block if it never was, so a host can scroll to a
	/// caret far outside the viewport.
	#[frb(sync)]
	pub fn caret_rect(&self) -> Result<Option<ViewRect>> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let Some(caret) = caret(&doc) else {
			return Ok(None);
		};
		state.lay_out_point(&doc, caret.point)?;
		Ok(state.layout.caret_rect(caret).map(ViewRect::from))
	}

	/// Where the selection's ends are, or `None` for a block selection.
	/// Lays out the blocks they are in, as [`CanvistView::caret_rect`] does.
	#[frb(sync)]
	pub fn selection_ends(&self) -> Result<Option<ViewSelectionEnds>> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let Some(range) = position::text_range(doc.document(), doc.selection()) else {
			return Ok(None);
		};
		state.lay_out_point(&doc, range.from)?;
		state.lay_out_point(&doc, range.to)?;
		let rect = |point| state.layout.caret_rect(Caret::new(point));

		Ok(rect(range.from).zip(rect(range.to)).map(|(start, end)| {
			ViewSelectionEnds {
				start: start.into(),
				end: end.into(),
				collapsed: range.is_empty(),
			}
		}))
	}

	/// The smallest rectangle around the selection.
	#[frb(sync)]
	pub fn selection_bounds(&self) -> Option<ViewRect> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		state
			.layout
			.selection_bounds(&doc.selection())
			.map(ViewRect::from)
	}

	/// The smallest rectangle around the IME composition.
	#[frb(sync)]
	pub fn composition_bounds(&self) -> Option<ViewRect> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let selection = composition_selection(&doc)?;
		state
			.layout
			.selection_bounds(&selection)
			.map(ViewRect::from)
	}

	/// The document's plain text and selection, for accessibility.
	#[frb(sync)]
	pub fn semantics(&self) -> ViewSemantics {
		let doc = lock(&self.doc);
		let document = doc.document();
		let (anchor, head) = selection_points(&doc);

		ViewSemantics {
			text: document.plain_text(),
			anchor: plain_offset(document, anchor),
			head: plain_offset(document, head),
		}
	}

	/// Select `anchor..head`, given in UTF-16 code units of
	/// [`ViewSemantics::text`], as assistive technology does.
	#[frb(sync)]
	pub fn select_plain(&self, anchor: u32, head: u32) -> Result<bool> {
		let selection = {
			let doc = lock(&self.doc);
			let document = doc.document();
			let (Some(anchor), Some(head)) =
				(plain_point(document, anchor), plain_point(document, head))
			else {
				return Ok(false);
			};
			Selection::text(anchor, head)
		};
		self.edit(|editor, env| editor.select(selection, env))
	}
}

impl CanvistView {
	/// The view's state and the document. Always the view first, then the
	/// document: document calls lock only the document, so the two can never
	/// wait on each other.
	fn lock(&self) -> (MutexGuard<'_, ViewState>, MutexGuard<'_, Doc>) {
		let state = self.state();
		(state, lock(&self.doc))
	}

	/// The view's state alone.
	fn state(&self) -> MutexGuard<'_, ViewState> {
		self.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
	}

	/// Make a change through the editor with the wall clock and this
	/// layout's line navigator, then bring the layout up to date.
	fn edit(
		&self,
		change: impl FnOnce(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> Result<bool> {
		let (mut state, mut doc) = self.lock();
		state.sync(&mut doc);
		let now = SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.unwrap_or(Duration::ZERO);
		let state = &mut *state;
		let mut lines = HostLines::new(&mut state.layout, &mut state.measurer);
		let update = doc.edit(now, &mut lines, change)?;
		state.sync(&mut doc);
		state.caret_on = true;

		Ok(update.is_some_and(|update| update.doc_changed() || update.selection_changed()))
	}

	/// The selection covering UTF-16 range `anchor..head` of the head's text
	/// block.
	fn block_selection(&self, anchor: u32, head: u32) -> Option<Selection> {
		let doc = lock(&self.doc);
		let (block, text) = head_block(&doc)?;

		Some(Selection::text(
			Point::new(block, byte_offset(&text, anchor)),
			Point::new(block, byte_offset(&text, head)),
		))
	}
}

impl ViewState {
	/// Pass every commit the document recorded to the layout.
	fn sync(&mut self, doc: &mut Doc) {
		let commits = doc.take_commits();

		for commit in &commits {
			self.layout.apply(doc.document(), commit);
		}
	}

	/// Lay out the block holding `point` if it has only an estimated height.
	fn lay_out_point(&mut self, doc: &Doc, point: Point) -> Result<()> {
		if self.layout.caret_rect(Caret::new(point)).is_some() {
			return Ok(());
		}

		let Some(rect) = self.layout.block_rect(point.block) else {
			return Ok(());
		};
		self.layout
			.ensure(
				doc.document(),
				&mut self.measurer,
				rect.y,
				rect.y + rect.height,
			)
			.map(drop)
			.map_err(|error| render(&error))
	}

	/// The model point at document coordinates `(x, y)`.
	fn point_at(&self, x: f32, y: f32) -> Option<Point> {
		self.layout.hit_test(x, y).map(|caret| caret.point)
	}

	/// What to draw between `top` and `bottom`: a caret only while focused
	/// and in the visible half of its blink, any other selection always.
	fn display_options(&self, doc: &Doc, top: f32, bottom: f32) -> DisplayOptions {
		let options = DisplayOptions::viewport(top, bottom);

		match caret(doc) {
			Some(caret) if self.focused && self.caret_on => options.caret(caret),
			Some(_) => options,
			None => options.selection(doc.selection()),
		}
	}

	/// `list` for the host, with the composition underlined.
	fn display(&self, doc: &Doc, list: &DisplayList) -> ViewDisplay {
		let mut items: Vec<ViewItem> = list.items.iter().map(item).collect();
		let chrome = self.theme.chrome();

		if let Some(selection) = composition_selection(doc) {
			for rect in self.layout.selection_rects(&selection) {
				let line = Rect::new(rect.x, rect.y + rect.height - 1.5, rect.width, 1.0);
				items.push(ViewItem::rect(
					ViewItemKind::Composition,
					line,
					chrome.caret,
				));
			}
		}

		ViewDisplay {
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
			items,
			background: argb(chrome.background),
		}
	}
}

/// Which way through the history a key goes.
enum History {
	Undo,
	Redo,
}

/// The history key `key` with `modifiers` is on a shared document, where
/// undo must go through the shared history: `Mod-z` undoes, `Mod-Shift-z`
/// and `Mod-y` redo, as the history extension binds them.
fn history_key(doc: &Doc, key: &str, modifiers: Modifiers) -> Option<History> {
	let primary = match doc.editor().platform() {
		Platform::Apple => modifiers.meta && !modifiers.ctrl,
		Platform::Other => modifiers.ctrl && !modifiers.meta,
	};

	if !doc.is_shared() || !primary || modifiers.alt {
		return None;
	}

	match (key.to_ascii_lowercase().as_str(), modifiers.shift) {
		("z", false) => Some(History::Undo),
		("z", true) | ("y", false) => Some(History::Redo),
		_ => None,
	}
}

/// A display item for the host.
pub(crate) fn item(item: &DisplayItem) -> ViewItem {
	match item {
		DisplayItem::Rect {
			rect,
			color,
			radius,
			kind,
			..
		} => {
			let kind = match kind {
				RectKind::Background => ViewItemKind::Background,
				RectKind::Border => ViewItemKind::Border,
				RectKind::Rule => ViewItemKind::Rule,
				RectKind::Highlight | RectKind::Range => ViewItemKind::Highlight,
				RectKind::Selection => ViewItemKind::Selection,
				RectKind::Caret => ViewItemKind::Caret,
			};
			ViewItem {
				radius: *radius,
				..ViewItem::rect(kind, *rect, *color)
			}
		}
		DisplayItem::Text(run) => {
			ViewItem {
				text: run.text().to_owned(),
				font: run.font.index() as u32,
				baseline: run.baseline,
				letter_spacing: run.letter_spacing,
				rtl: run.rtl,
				..ViewItem::rect(ViewItemKind::Text, run.bounds, run.color)
			}
		}
		DisplayItem::Decoration {
			kind,
			x,
			y,
			width,
			thickness,
			color,
			..
		} => {
			let (kind, top) = match kind {
				DecorationKind::Underline => (ViewItemKind::Underline, y - thickness / 2.0),
				DecorationKind::Strike => (ViewItemKind::Strike, y - thickness / 2.0),
				DecorationKind::Squiggle => (ViewItemKind::Squiggle, *y),
			};
			ViewItem::rect(kind, Rect::new(*x, top, *width, *thickness), *color)
		}
		DisplayItem::Stroke {
			points,
			width,
			color,
			..
		} => {
			ViewItem {
				points: points.to_vec(),
				line_width: *width,
				..ViewItem::rect(ViewItemKind::Stroke, item.bounds(), *color)
			}
		}
		DisplayItem::Slot { kind, rect, .. } => {
			let (kind, checked) = match kind {
				SlotKind::Checkbox { checked } => (ViewItemKind::Checkbox, *checked),
				SlotKind::Block | SlotKind::Embed(_) => (ViewItemKind::Slot, false),
			};
			ViewItem {
				checked,
				..ViewItem::rect(kind, *rect, Color::TRANSPARENT)
			}
		}
	}
}

/// A colour as `0xAARRGGBB`, the layout `dart:ui` colours use.
pub(crate) fn argb(color: Color) -> u32 {
	u32::from_be_bytes([color.a, color.r, color.g, color.b])
}

fn lock(doc: &Mutex<Doc>) -> MutexGuard<'_, Doc> {
	// A panic while holding the lock leaves the document as the failed call
	// left it, which every call is written to tolerate.
	doc.lock()
		.unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn render(error: &LayoutError) -> CanvistFailure {
	CanvistFailure {
		code: "render".to_owned(),
		message: error.to_string(),
	}
}

fn parse_json(text: &str) -> Result<Json> {
	serde_json::from_str(text).map_err(|error| {
		CanvistFailure::from(HeadlessError::InvalidArgs {
			name: "json".to_owned(),
			reason: error.to_string(),
		})
	})
}

/// The caret, when the selection is one.
fn caret(doc: &Doc) -> Option<Caret> {
	match doc.selection() {
		Selection::Text { anchor, head } if anchor == head => {
			let affinity = doc.state().affinity().into();
			Some(Caret::with_affinity(head, affinity))
		}
		_ => None,
	}
}

/// The IME composition as a text selection.
fn composition_selection(doc: &Doc) -> Option<Selection> {
	let composition = doc.state().composition()?;
	(composition.start != composition.end)
		.then(|| Selection::text(composition.start_point(), composition.end_point()))
}

/// The text block holding the selection's head, with its text.
fn head_block(doc: &Doc) -> Option<(BlockId, String)> {
	let (_, head) = selection_points(doc);
	let text = doc.document().inline(head.block)?.as_str().to_owned();
	Some((head.block, text))
}

/// The anchor and head of a text selection, or the start of a block
/// selection.
fn selection_points(doc: &Doc) -> (Point, Point) {
	let document = doc.document();

	match doc.selection() {
		Selection::Text { anchor, head } => (anchor, head),
		selection @ Selection::Blocks { .. } => {
			let point = blocks_start(document, selection);
			(point, point)
		}
	}
}

/// Where a block selection starts: its first text point, or the document's.
fn blocks_start(document: &Document, selection: Selection) -> Point {
	if let Some(range) = position::text_range(document, selection) {
		return range.from;
	}

	match position::start_selection(document) {
		Selection::Text { head, .. } => head,
		Selection::Blocks { .. } => Point::new(document.children(None)[0], 0),
	}
}

fn selection_is_collapsed(doc: &Doc) -> bool {
	position::text_range(doc.document(), doc.selection()).is_none_or(|range| range.is_empty())
}

/// The range `unit` covers around `point`.
fn unit_around(doc: &Doc, point: Point, unit: Unit) -> (Point, Point) {
	let Some(text) = doc.document().inline(point.block).map(InlineText::as_str) else {
		return (point, point);
	};

	match unit {
		Unit::Character => (point, point),
		Unit::Word => {
			let (start, end) = word_around(text, point.offset);
			(Point::new(point.block, start), Point::new(point.block, end))
		}
		Unit::Block => {
			(
				Point::new(point.block, 0),
				Point::new(point.block, text.len() as u32),
			)
		}
	}
}

fn clipboard_formats(document: &Document, slice: &Slice) -> ViewClipboard {
	let schema = document.schema();
	ViewClipboard {
		html: clipboard::to_html(schema, slice),
		text: clipboard::to_plain_text(schema, slice),
	}
}

/// Where `point` is in [`Document::plain_text`], in UTF-16 code units.
fn plain_offset(document: &Document, point: Point) -> u32 {
	let mut offset = 0;

	for block in document.text_blocks() {
		let text = block.inline().map_or("", InlineText::as_str);

		if block.id() == point.block {
			return offset + utf16(text, point.offset);
		}

		offset += text.encode_utf16().count() as u32 + 1;
	}

	offset.saturating_sub(1)
}

/// The point at UTF-16 offset `units` of [`Document::plain_text`].
fn plain_point(document: &Document, units: u32) -> Option<Point> {
	let mut remaining = units;
	let mut last = None;

	for block in document.text_blocks() {
		let text = block.inline().map_or("", InlineText::as_str);
		let length = text.encode_utf16().count() as u32;

		if remaining <= length {
			return Some(Point::new(block.id(), byte_offset(text, remaining)));
		}

		remaining -= length + 1;
		last = Some(Point::new(block.id(), text.len() as u32));
	}

	last
}

/// The UTF-16 offset of byte `offset` in `text`.
fn utf16(text: &str, offset: u32) -> u32 {
	text.get(..offset as usize)
		.map_or(0, |head| head.encode_utf16().count() as u32)
}

/// The byte offset of UTF-16 offset `units` in `text`, snapped back to a
/// character boundary.
fn byte_offset(text: &str, units: u32) -> u32 {
	let mut counted = 0;

	for (offset, character) in text.char_indices() {
		if counted >= units {
			return offset as u32;
		}

		counted += character.len_utf16() as u32;
	}

	text.len() as u32
}

/// The word, run of spaces, or punctuation that `offset` is in or just
/// before, as a byte range.
fn word_around(text: &str, offset: u32) -> (u32, u32) {
	let offset = offset as usize;
	let mut last = (0, 0);

	for (start, segment) in text.split_word_bound_indices() {
		let end = start + segment.len();
		last = (start as u32, end as u32);

		if offset < end {
			return last;
		}
	}

	last
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn utf16_offsets_round_trip() {
		let text = "a😀b";

		assert_eq!(utf16(text, 5), 3);
		assert_eq!(byte_offset(text, 3), 5);
		assert_eq!(byte_offset(text, 99), text.len() as u32);
	}

	#[test]
	fn words_around_offsets() {
		let text = "hello, wide world";

		assert_eq!(word_around(text, 2), (0, 5));
		assert_eq!(word_around(text, 5), (5, 6), "the comma");
		assert_eq!(word_around(text, text.len() as u32), (12, 17));
	}

	#[test]
	fn colours_cross_as_argb() {
		assert_eq!(
			argb(Color {
				r: 0x11,
				g: 0x22,
				b: 0x33,
				a: 0x80
			}),
			0x8011_2233
		);
	}
}
