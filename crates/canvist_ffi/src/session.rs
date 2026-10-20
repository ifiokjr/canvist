//! One editor's state: the document, its layout, and what is on screen.
//!
//! Every change goes through [`Session::edit`] (or a [`Doc`] call followed
//! by [`Session::sync`]), so key bindings, input rules, history, and
//! collaboration apply, and layout and the text index see every commit.

use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::canvist_editor::CommandResult;
use canvist_headless::canvist_editor::Editor as CoreEditor;
use canvist_headless::canvist_editor::Env;
use canvist_headless::canvist_editor::KeyPress;
use canvist_headless::canvist_editor::LineNavigator;
use canvist_headless::canvist_editor::Modifiers as KeyModifiers;
use canvist_headless::canvist_editor::command;
use canvist_headless::canvist_editor::commands;
use canvist_headless::canvist_editor::position;
use canvist_headless::canvist_model::Point;
use canvist_headless::canvist_model::Selection;
use canvist_layout::Affinity;
use canvist_layout::Caret;
use canvist_layout::ColorScheme;
use canvist_layout::DecorationKind;
use canvist_layout::DisplayItem as LayoutItem;
use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::LayoutOptions;
use canvist_layout::StarterKitOptions;
use canvist_layout::Theme;

use crate::Bounds;
use crate::CanvistError;
use crate::EditorChange;
use crate::EditorOptions;
use crate::Frame;
use crate::Modifiers;
use crate::OffsetRange;
use crate::TextSelection;
use crate::ThemeOptions;
use crate::display;
use crate::error::Result;
use crate::lines::HostLines;
use crate::measure::HostMeasurer;
use crate::text::TextIndex;

/// More dirty rectangles than this repaint everything instead.
const MAX_DAMAGE: usize = 16;

/// The marked-text underline, as a share of the line height.
const COMPOSITION_THICKNESS: f32 = 0.06;

/// What [`Session::change_since`] compares to report an [`EditorChange`].
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Observed {
	version: u64,
	selection: TextSelection,
	composition: Option<OffsetRange>,
}

#[allow(
	clippy::struct_excessive_bools,
	reason = "independent facts about the view: focus, blink, and what to repaint"
)]
pub(crate) struct Session {
	engine: Engine,
	pub(crate) doc: Doc,
	theme_options: ThemeOptions,
	theme: Arc<Theme>,
	overscan: f32,
	draws_selection: bool,
	pub(crate) layout: DocumentLayout,
	pub(crate) measurer: HostMeasurer,
	pub(crate) text: TextIndex,
	/// A composing region an Android input method marked without
	/// replacing it yet, in editor text offsets.
	region: Option<OffsetRange>,
	/// The selection an input method made inside the marked text, such as
	/// the clause a Japanese IME is converting. The model's caret stays at
	/// the end of the marked text, so moving it does not split the
	/// composition's undo step. It holds while the document stays at the
	/// version it was made at.
	marked_selection: Option<(u64, TextSelection)>,
	/// The viewport: its height and the document coordinate at its top.
	height: f32,
	top: f32,
	focused: bool,
	caret_on: bool,
	/// What the platform shows now, for the next frame's damage.
	painted: Option<DisplayList>,
	full_repaint: bool,
}

impl Session {
	pub(crate) fn new(options: &EditorOptions, measurer: HostMeasurer) -> Result<Self> {
		let theme = theme(&options.theme)?;
		let engine = Engine::named(&options.extensions)?
			.with_client(options.client)
			.with_platform(options.platform.into());
		let mut doc = engine.create()?;
		doc.record_commits();
		let overscan = finite_or_zero(options.overscan);
		let layout = DocumentLayout::new(
			doc.document(),
			theme.clone(),
			LayoutOptions::new(0.0).overscan(overscan),
		);
		let text = TextIndex::new(doc.document());

		Ok(Self {
			engine,
			doc,
			theme_options: options.theme.clone(),
			theme,
			overscan,
			draws_selection: options.draws_selection,
			layout,
			measurer,
			text,
			region: None,
			marked_selection: None,
			height: 0.0,
			top: 0.0,
			focused: false,
			caret_on: true,
			painted: None,
			full_repaint: true,
		})
	}

	pub(crate) fn engine(&self) -> &Engine {
		&self.engine
	}

	/// Replace the document, keeping the viewport.
	pub(crate) fn set_doc(&mut self, mut doc: Doc) {
		doc.record_commits();
		let width = self.layout.options().width;
		self.doc = doc;
		self.layout = DocumentLayout::new(
			self.doc.document(),
			self.theme.clone(),
			LayoutOptions::new(width).overscan(self.overscan),
		);
		self.measurer.reset_fonts();
		self.text = TextIndex::new(self.doc.document());
		self.region = None;
		self.marked_selection = None;

		self.painted = None;
		self.full_repaint = true;
	}

	/// Change the document through the editor, with the wall clock (so
	/// typing groups into undo steps by real time) and the layout's line
	/// navigator (so `ArrowUp` moves by visual lines). Returns whether the
	/// document or selection changed.
	pub(crate) fn edit(
		&mut self,
		change: impl FnOnce(&mut CoreEditor, &mut Env<'_>) -> CommandResult,
	) -> Result<bool> {
		let mut lines = HostLines::new(&mut self.layout, &mut self.measurer);
		let update = self.doc.edit(now(), &mut lines, change);
		self.sync();
		let update = update?;
		Ok(update.is_some_and(|update| update.doc_changed() || update.selection_changed()))
	}

	/// Let layout and the text index account for every commit since the
	/// last call.
	pub(crate) fn sync(&mut self) {
		let commits = self.doc.take_commits();

		for commit in &commits {
			let damage = self.layout.apply(self.doc.document(), commit);
			self.full_repaint |= damage.from_y.is_some();
			self.text.apply(self.doc.document(), commit);
		}

		if !commits.is_empty() {
			self.region = None;
		}

		self.caret_on = true;
	}

	/// Run a [`Doc`] call that changes the document outside
	/// [`Session::edit`], such as shared undo or a Yjs update.
	pub(crate) fn with_doc<T>(
		&mut self,
		call: impl FnOnce(&mut Doc) -> std::result::Result<T, canvist_headless::HeadlessError>,
	) -> Result<T> {
		let result = call(&mut self.doc);
		self.sync();
		Ok(result?)
	}

	pub(crate) fn press(&mut self, key: &str, modifiers: Modifiers) -> Result<bool> {
		let modifiers = KeyModifiers {
			shift: modifiers.shift,
			ctrl: modifiers.ctrl,
			alt: modifiers.alt,
			meta: modifiers.meta,
		};
		self.edit(|editor, env| editor.handle_key(KeyPress::new(key, modifiers), env))
	}

	pub(crate) fn observed(&self) -> Observed {
		Observed {
			version: self.doc.document().version(),
			selection: self.selection(),
			composition: self.marked_range(),
		}
	}

	pub(crate) fn change_since(&self, before: &Observed) -> EditorChange {
		let now = self.observed();
		let document = now.version != before.version;

		EditorChange {
			document,
			selection: now.selection != before.selection,
			composition: now.composition != before.composition,
			updates: document && self.doc.is_shared(),
		}
	}

	pub(crate) fn selection(&self) -> TextSelection {
		if let Some(selection) = self.marked_selection() {
			return selection;
		}

		let doc = self.doc.document();
		let (anchor, head) = self.selection_points();
		let offset = |point| self.text.offset(doc, point).unwrap_or(0);

		TextSelection {
			anchor: offset(anchor),
			head: offset(head),
		}
	}

	/// The selection inside the marked text, while it still applies.
	fn marked_selection(&self) -> Option<TextSelection> {
		let (version, selection) = self.marked_selection?;
		let current =
			version == self.doc.document().version() && self.doc.state().composition().is_some();
		current.then_some(selection)
	}

	/// The anchor and head of a text selection; a block selection as the
	/// text range it covers, or the document's start.
	fn selection_points(&self) -> (Point, Point) {
		let doc = self.doc.document();
		let selection = self.doc.selection();

		if let Selection::Text { anchor, head } = selection {
			return (anchor, head);
		}

		let covered = position::text_range(doc, selection)
			.or_else(|| position::text_range(doc, position::start_selection(doc)));

		if let Some(range) = covered {
			return (range.from, range.to);
		}

		let start = self
			.point(0)
			.unwrap_or(Point::new(doc.children(None)[0], 0));
		(start, start)
	}

	pub(crate) fn point(&self, offset: u32) -> Option<Point> {
		self.text.point(self.doc.document(), offset)
	}

	pub(crate) fn select(&mut self, anchor: u32, head: u32) -> Result<bool> {
		self.marked_selection = None;
		let (Some(anchor), Some(head)) = (self.point(anchor), self.point(head)) else {
			return Ok(false);
		};
		self.select_points(anchor, head)
	}

	fn select_points(&mut self, anchor: Point, head: Point) -> Result<bool> {
		let selection = Selection::text(anchor, head);

		if selection == self.doc.selection() {
			return Ok(false);
		}

		self.edit(|editor, env| editor.select(selection, env))
	}

	/// Type `text` over the selection. Line breaks press Enter, so lists and
	/// code blocks continue the way a keyboard's Return does.
	pub(crate) fn type_text(&mut self, text: &str) -> Result<bool> {
		let mut changed = false;
		let text = text.replace("\r\n", "\n");

		for (index, line) in text.split(['\n', '\r']).enumerate() {
			if index > 0 {
				changed |= self.press("Enter", Modifiers::default())?;
			}

			if !line.is_empty() {
				changed |= self.edit(|editor, env| editor.insert_text(line, env))?;
			}
		}

		Ok(changed)
	}

	/// Replace `range` with `text`, as an input method does. Typing at a
	/// caret runs input rules; an empty `text` deletes.
	pub(crate) fn replace(&mut self, range: OffsetRange, text: &str) -> Result<bool> {
		let moved = self.select(range.start, range.end)?;

		if !text.is_empty() {
			return Ok(self.type_text(text)? || moved);
		}

		if range.start == range.end {
			return Ok(moved);
		}

		let deleted =
			self.edit(|editor, env| editor.run(&command(commands::delete_selection), env))?;
		Ok(moved || deleted)
	}

	/// Insert `text` at the selection: commits marked text when there is
	/// some, as `UITextInput.insertText` and `InputConnection.commitText` do.
	pub(crate) fn insert_text(&mut self, text: &str) -> Result<bool> {
		self.marked_selection = None;

		if self.doc.state().composition().is_some() {
			let committed = self.edit(|editor, env| editor.commit_composition(text, env))?;

			return Ok(committed);
		}

		if let Some(region) = self.region.take() {
			return self.replace(region, text);
		}

		self.type_text(text)
	}

	/// The marked text of a composition, or the composing region an input
	/// method marked.
	pub(crate) fn marked_range(&self) -> Option<OffsetRange> {
		let doc = self.doc.document();

		match self.doc.state().composition() {
			Some(composition) => {
				let start = self.text.offset(doc, composition.start_point())?;
				let end = self.text.offset(doc, composition.end_point())?;
				Some(OffsetRange::between(start, end))
			}
			None => self.region,
		}
	}

	/// Show `text` as marked text in place of the marked text, the composing
	/// region, or the selection, with `selected` (relative to `text`)
	/// selected inside it.
	pub(crate) fn set_marked_text(&mut self, text: &str, selected: OffsetRange) -> Result<bool> {
		let mut changed = false;

		if self.doc.state().composition().is_none()
			&& let Some(region) = self.region.take()
		{
			changed |= self.select(region.start, region.end)?;
		}

		changed |= self.edit(|editor, env| editor.set_composition(text, env))?;

		let Some(marked) = self.marked_range() else {
			self.marked_selection = None;

			return Ok(changed);
		};
		let length = marked.end - marked.start;
		let selection = TextSelection {
			anchor: marked.start + selected.start.min(length),
			head: marked.start + selected.end.min(length),
		};
		let inside = selection
			!= TextSelection {
				anchor: marked.end,
				head: marked.end,
			};
		let marked_selection = inside.then_some((self.doc.document().version(), selection));
		changed |= marked_selection != self.marked_selection;

		self.marked_selection = marked_selection;

		Ok(changed)
	}

	/// Commit the marked text as it is.
	pub(crate) fn unmark_text(&mut self) -> Result<bool> {
		self.region = None;

		let Some(marked) = self.marked_range() else {
			return Ok(false);
		};
		let text = self.text.text(self.doc.document(), marked);
		let inside = self.marked_selection();
		self.marked_selection = None;
		let committed = self.edit(|editor, env| editor.commit_composition(&text, env))?;

		// Committing leaves the caret after the text; keep the selection the
		// input method had made inside it.
		let moved = match inside {
			Some(selection) => self.select(selection.anchor, selection.head)?,
			None => false,
		};

		Ok(committed || moved)
	}

	/// Mark `range` as the text an input method is composing, without
	/// changing it, or forget the region.
	pub(crate) fn set_composing_region(&mut self, range: Option<OffsetRange>) -> Result<bool> {
		if self.doc.state().composition().is_some() {
			self.unmark_text()?;
		}

		self.marked_selection = None;
		let range = range.map(|range| {
			let length = self.text.len();
			OffsetRange::between(range.start.min(length), range.end.min(length))
		});
		let changed = range != self.region;
		self.region = range.filter(|range| range.start < range.end);
		self.full_repaint |= changed;
		Ok(changed)
	}

	/// Lay out the viewport, the selection's blocks, and the blocks holding
	/// `offsets`, so geometry describes the document as it is now: an edit
	/// leaves the blocks it touched unmeasured until the next frame, and
	/// platforms ask for the caret straight after typing. A measurer that
	/// breaks its contract fails the next frame instead.
	pub(crate) fn lay_out(&mut self, offsets: &[u32]) {
		let selection = self.selection();
		let mut spans = vec![(self.top, self.top + self.height)];

		for &offset in offsets.iter().chain([&selection.anchor, &selection.head]) {
			let block = self
				.point(offset)
				.and_then(|point| self.layout.block_rect(point.block));
			spans.extend(block.map(|rect| (rect.y, rect.y + rect.height)));
		}

		for (top, bottom) in spans {
			let laid_out = self
				.layout
				.ensure(self.doc.document(), &mut self.measurer, top, bottom);

			if laid_out.is_err() {
				return;
			}
		}
	}

	/// The caret rectangle at `offset`. At the selection's head the caret
	/// keeps its affinity, so it shows at the end of a soft-wrapped line
	/// when it got there from that line.
	pub(crate) fn caret_rect(&self, offset: u32) -> Option<Bounds> {
		let point = self.point(offset)?;
		let affinity = if offset == self.selection().head {
			self.doc.state().affinity().into()
		} else {
			Affinity::Downstream
		};

		self.layout
			.caret_rect(Caret::with_affinity(point, affinity))
			.map(Bounds::from)
	}

	pub(crate) fn selection_rects(&self, range: OffsetRange) -> Vec<Bounds> {
		let (Some(from), Some(to)) = (self.point(range.start), self.point(range.end)) else {
			return Vec::new();
		};
		self.layout
			.selection_rects(&Selection::text(from, to))
			.into_iter()
			.map(Bounds::from)
			.collect()
	}

	pub(crate) fn offset_at(&self, x: f32, y: f32) -> Option<u32> {
		let caret = self.layout.hit_test(x, y)?;
		self.text.offset(self.doc.document(), caret.point)
	}

	pub(crate) fn set_viewport(&mut self, width: f32, height: f32) {
		let width = finite_or_zero(width).max(0.0);

		if width.to_bits() != self.layout.options().width.to_bits() {
			self.layout.set_width(self.doc.document(), width);
			self.full_repaint = true;
		}

		self.height = finite_or_zero(height).max(0.0);
	}

	pub(crate) fn set_scroll(&mut self, top: f32) {
		let top = finite_or_zero(top);

		if top.to_bits() != self.top.to_bits() {
			self.top = top;
			self.full_repaint = true;
		}
	}

	pub(crate) fn set_focused(&mut self, focused: bool) {
		self.focused = focused;
		self.caret_on = true;
	}

	pub(crate) fn set_caret_visible(&mut self, visible: bool) {
		self.caret_on = visible;
	}

	pub(crate) fn set_theme(&mut self, options: &ThemeOptions) -> Result<()> {
		if *options == self.theme_options {
			return Ok(());
		}

		self.theme = theme(options)?;
		self.theme_options = options.clone();
		self.layout
			.set_theme(self.doc.document(), self.theme.clone());
		self.full_repaint = true;
		Ok(())
	}

	/// Forget every measurement, as when the platform's fonts change.
	pub(crate) fn forget_measurements(&mut self) {
		self.layout.forget_measurements();
		self.full_repaint = true;
	}

	pub(crate) fn height(&self) -> f32 {
		self.layout.height()
	}

	/// Lay out the viewport and report what to paint.
	pub(crate) fn frame(&mut self) -> Result<Frame> {
		let bottom = self.top + self.height;
		let update =
			self.layout
				.ensure(self.doc.document(), &mut self.measurer, self.top, bottom)?;
		let mut list = self.layout.display_list(&self.display_options(bottom));
		self.underline_marked_text(&mut list);

		let mut full = self.full_repaint || update.shift != 0.0;

		let damage = match self.painted.as_ref() {
			Some(previous) if !full => list.damage(previous),
			_ => {
				full = true;
				Vec::new()
			}
		};

		full |= damage.len() > MAX_DAMAGE;
		let items =
			(full || !damage.is_empty()).then(|| display::items(&list, self.doc.document()));
		let damage = if full {
			Vec::new()
		} else {
			damage.into_iter().map(Bounds::from).collect()
		};

		let frame = Frame {
			height: self.layout.height(),
			shift: update.shift,
			background: self.theme.chrome().background.into(),
			items,
			damage,
			full,
			laid_out: update.laid_out as u32,
			measured: update.measurer_calls as u32,
		};

		self.painted = Some(list);
		self.full_repaint = false;
		Ok(frame)
	}

	/// The caret while it is shown, or the selection highlight, when this
	/// editor draws them.
	fn display_options(&self, bottom: f32) -> DisplayOptions {
		let options = DisplayOptions::viewport(self.top, bottom);

		if !self.draws_selection {
			return options;
		}

		let selection = self.doc.selection();
		let Some(caret) = selection.as_caret() else {
			return options.selection(selection);
		};

		if !(self.focused && self.caret_on) {
			return options;
		}

		let affinity = self.doc.state().affinity().into();
		options.caret(Caret::with_affinity(caret, affinity))
	}

	/// Underline the marked text, as platform text fields do.
	fn underline_marked_text(&self, list: &mut DisplayList) {
		let Some(marked) = self.marked_range().filter(|range| range.start < range.end) else {
			return;
		};
		let (Some(from), Some(to)) = (self.point(marked.start), self.point(marked.end)) else {
			return;
		};
		let color = self.theme.chrome().caret;

		for rect in self.layout.selection_rects(&Selection::text(from, to)) {
			let thickness = (rect.height * COMPOSITION_THICKNESS).max(1.0);
			list.items.push(LayoutItem::Decoration {
				kind: DecorationKind::Underline,
				x: rect.x,
				y: rect.y + rect.height - thickness,
				width: rect.width,
				thickness,
				color,
				block: from.block,
			});
		}
	}

	/// Ask the line navigator about `caret` in the current document.
	pub(crate) fn navigate<T>(
		&mut self,
		ask: impl FnOnce(
			&mut dyn LineNavigator,
			&canvist_headless::canvist_model::Document,
		) -> Option<T>,
	) -> Option<T> {
		let mut lines = HostLines::new(&mut self.layout, &mut self.measurer);
		ask(&mut lines, self.doc.document())
	}
}

/// The StarterKit theme for `options`.
fn theme(options: &ThemeOptions) -> Result<Arc<Theme>> {
	let positive = |value: f32| value.is_finite() && value > 0.0;

	if !positive(options.font_size) || !positive(options.line_height) {
		return Err(CanvistError::invalid_args(
			"theme",
			"font_size and line_height must be positive",
		));
	}

	Ok(Arc::new(Theme::starter_kit_with(&StarterKitOptions {
		scheme: if options.dark {
			ColorScheme::Dark
		} else {
			ColorScheme::Light
		},

		body_family: Arc::from(options.body_family.as_str()),
		code_family: Arc::from(options.code_family.as_str()),
		size: options.font_size,
		line_height: options.line_height,
		letter_spacing: 0.0,
	})))
}

/// The wall clock, for grouping typing into undo steps.
fn now() -> Duration {
	SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap_or_default()
}

fn finite_or_zero(value: f32) -> f32 {
	if value.is_finite() { value } else { 0.0 }
}
