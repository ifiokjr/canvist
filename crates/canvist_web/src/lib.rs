#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_web`
//!
//! The canvist editor in the browser: a [`canvist_headless::Doc`] laid out
//! by [`canvist_layout`] and painted on an opaque Canvas2D context. The
//! `@canvist/web` package wraps [`WebEditor`] in a typed API and owns
//! everything DOM-shaped: sizing, scrolling, the frame loop, input, and
//! fonts.
//!
//! Text is measured by the browser itself, through the host's
//! [`HostMeasurer`], with the same font strings and text properties the
//! canvas paints with and the server-rendered HTML declares. That is what
//! makes the canvas wrap every line where the HTML does, so swapping one
//! for the other moves nothing.
//!
//! A frame is cheap when little changed: layout relays out only the blocks
//! a commit touched, the display list covers only the viewport, and the
//! painter repaints only the rectangles that differ from the last frame.
#[cfg(feature = "collab")]
mod collab;
mod error;
mod geometry;
mod input;
mod lines;
mod measure;
mod options;
mod paint;

use std::time::Duration;

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Position;
use canvist_headless::canvist_editor::CommandResult;
use canvist_headless::canvist_editor::Editor;
use canvist_headless::canvist_editor::Env;
use canvist_headless::canvist_editor::Update;
use canvist_headless::canvist_model::Commit;
use canvist_headless::canvist_model::Selection;
use canvist_layout::Caret;
use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::HtmlOptions;
use canvist_layout::LayoutOptions;
use canvist_layout::Rect;
use serde_json::Value as Json;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::error::JsResult;
use crate::error::headless;
use crate::error::invalid_args;
use crate::error::invalid_options;
use crate::error::render;
use crate::lines::HostLines;
use crate::measure::BrowserMeasurer;
pub use crate::measure::HostMeasurer;
use crate::options::Options;
use crate::paint::Painter;
use crate::paint::PixelGrid;
use crate::paint::Region;
use crate::paint::Surface;

/// More dirty rectangles than this repaint the whole canvas instead: past
/// a handful, clipping costs more than it saves.
const MAX_DIRTY_RECTS: usize = 16;

/// What one [`WebEditor::frame`] did. It crosses to JavaScript as a
/// `Float64Array` (see [`Frame::to_array`]): a class would cost the bundle
/// a wrapper, a finalizer, and a getter per field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Frame {
	/// The document's height in CSS pixels, including estimates for blocks
	/// not laid out yet. The host sizes its scroll area to it.
	pub height: f32,
	/// How far content at the top of the viewport moved because blocks above
	/// it were measured. Add it to the scroll position to keep the content
	/// still.
	pub shift: f32,
	/// Blocks laid out this frame.
	pub laid_out: u32,
	/// Measurer calls this frame: 0 when every segment was cached.
	pub measured: u32,
	/// Display items drawn.
	pub drawn: u32,
	/// Whether the whole canvas was repainted.
	pub full: bool,
	/// Whether anything was painted at all.
	pub painted: bool,
}

impl Frame {
	/// `[height, shift, laidOut, measured, drawn, full, painted]`, the two
	/// flags as 0 or 1.
	fn to_array(self) -> Vec<f64> {
		vec![
			f64::from(self.height),
			f64::from(self.shift),
			f64::from(self.laid_out),
			f64::from(self.measured),
			f64::from(self.drawn),
			f64::from(u8::from(self.full)),
			f64::from(u8::from(self.painted)),
		]
	}
}

/// One editor on one canvas.
#[wasm_bindgen]
pub struct WebEditor {
	engine: Engine,
	doc: Doc,
	options: Options,
	layout: DocumentLayout,
	measurer: BrowserMeasurer,
	painter: Painter,
	/// What the canvas shows now, for computing the next frame's damage.
	painted: Option<DisplayList>,
	/// Damage that the display list diff cannot see, such as glyphs that
	/// changed shape when a font loaded: repainted on the next frame.
	dirty: Vec<Rect>,
	full_repaint: bool,
	/// The document coordinate at the canvas's top edge.
	top: f32,
	/// Whether the caret is shown at all (the editor has focus) and whether
	/// it is in the visible half of its blink.
	focused: bool,
	caret_on: bool,
	/// The pointer drag selecting text, while the pointer is pressed.
	drag: Option<input::Drag>,
	/// The yrs client id this editor joins a shared document as, until the
	/// server's state arrives.
	#[cfg(feature = "collab")]
	joining: Option<u32>,
}

#[wasm_bindgen]
impl WebEditor {
	/// An editor painting on `canvas`, measuring through `measurer`, with the
	/// named extensions and JSON `options` (see `@canvist/web`'s
	/// `EditorOptions`).
	#[wasm_bindgen(constructor)]
	#[allow(
		clippy::needless_pass_by_value,
		reason = "wasm-bindgen hands JavaScript values over by value"
	)]
	pub fn new(
		canvas: HtmlCanvasElement,
		measurer: HostMeasurer,
		extensions: Vec<String>,
		options: &str,
	) -> JsResult<WebEditor> {
		let json: Json =
			serde_json::from_str(options).map_err(|error| invalid_options(&error.to_string()))?;
		let options = options::parse(&json).map_err(|reason| invalid_options(&reason))?;
		let engine = Engine::named(&extensions)
			.map_err(headless)?
			.with_client(options.client)
			.with_platform(options.platform);
		let doc = engine.create().map_err(headless)?;
		let layout = DocumentLayout::new(
			doc.document(),
			options.theme.clone(),
			layout_options(&options, 0.0),
		);

		Ok(Self {
			engine,
			doc,
			options,
			layout,
			measurer: BrowserMeasurer::new(measurer),
			painter: Painter::new(canvas)?,
			painted: None,
			dirty: Vec::new(),
			full_repaint: true,
			top: 0.0,
			focused: false,
			caret_on: true,
			drag: None,
			#[cfg(feature = "collab")]
			joining: None,
		})
	}

	/// Replace the document with `input` read as `format` (`json`, `html`,
	/// `markdown`, or `text`).
	pub fn load(&mut self, format: &str, input: &str) -> JsResult<()> {
		let format: Format = format.parse().map_err(headless)?;
		let doc = self.engine.parse(format, input).map_err(headless)?;
		self.replace(doc);

		Ok(())
	}

	/// The document as `format`.
	pub fn export(&self, format: &str) -> JsResult<String> {
		let format: Format = format.parse().map_err(headless)?;
		Ok(self.doc.export(format))
	}

	/// Server-rendered HTML for the document at the current width, as
	/// `[fragment, stylesheet]`: what SSR sends down, and what the
	/// accessibility mirror shows.
	pub fn html(&self) -> Vec<String> {
		let html = canvist_layout::to_html(
			self.doc.document(),
			&self.options.theme,
			&HtmlOptions::new(self.layout.options().width),
		);
		vec![html.html, html.css]
	}

	/// Size the canvas: `width` and `height` in CSS pixels, `scale` device
	/// pixels per CSS pixel, and the backing store in device pixels. The
	/// document is laid out to `width`.
	pub fn resize(
		&mut self,
		width: f32,
		height: f32,
		scale: f32,
		pixel_width: u32,
		pixel_height: u32,
	) {
		let resized = self.painter.resize(Surface {
			width,
			height,
			scale,
			pixel_width,
			pixel_height,
		});

		if resized {
			self.full_repaint = true;
		}

		if width.to_bits() != self.layout.options().width.to_bits() {
			self.layout.set_width(self.doc.document(), width);
			self.full_repaint = true;
		}
	}

	/// Scroll so document coordinate `top` is at the canvas's top edge.
	#[wasm_bindgen(js_name = setTop)]
	pub fn set_top(&mut self, top: f32) {
		if top.to_bits() != self.top.to_bits() {
			self.top = top;
			self.full_repaint = true;
		}
	}

	/// Show or hide the caret: hosts call this on focus and blur.
	#[wasm_bindgen(js_name = setFocused)]
	pub fn set_focused(&mut self, focused: bool) {
		self.focused = focused;
		self.caret_on = true;
	}

	/// Blink the caret.
	#[wasm_bindgen(js_name = setCaretOn)]
	pub fn set_caret_on(&mut self, on: bool) {
		self.caret_on = on;
	}

	/// Lay out the viewport and paint whatever changed since the last frame.
	/// Returns what it did as `[height, shift, laidOut, measured, drawn,
	/// full, painted]`: `height` is the document's height in CSS px, `shift`
	/// how far content at the viewport's top moved (add it to the scroll
	/// position), and the flags are 0 or 1.
	pub fn frame(&mut self) -> JsResult<Vec<f64>> {
		let surface = self.painter.surface();
		let bottom = self.top + surface.height;
		let update = self
			.layout
			.ensure(self.doc.document(), &mut self.measurer, self.top, bottom)
			.map_err(|error| render(&error))?;
		let list = self.layout.display_list(&self.display_options(bottom));
		let mut frame = Frame {
			height: self.layout.height(),
			shift: update.shift,
			laid_out: update.laid_out as u32,
			measured: update.measurer_calls as u32,
			..Frame::default()
		};

		let full = self.full_repaint || update.shift != 0.0;
		let damage = self.damage(&list, full);

		if full || damage.len() > MAX_DIRTY_RECTS {
			self.paint(&list, &Region::All);
			frame.full = true;
			frame.painted = true;
		} else if !damage.is_empty() {
			self.paint(&list, &Region::Rects(&damage));
			frame.painted = true;
		}

		frame.drawn = self.painter.drawn();
		self.dirty.clear();
		self.full_repaint = false;
		self.painted = Some(list);
		Ok(frame.to_array())
	}

	/// Font faces of `families` finished loading: forget only the
	/// measurements of fonts that list one of them, so the blocks using them
	/// lay out again on the next frame. Returns how many measured blocks were
	/// invalidated.
	#[wasm_bindgen(js_name = fontsLoaded)]
	#[allow(
		clippy::needless_pass_by_value,
		reason = "wasm-bindgen hands JavaScript arrays over by value"
	)]
	pub fn fonts_loaded(&mut self, families: Vec<String>) -> u32 {
		let mut invalidation = self
			.layout
			.forget_fonts(|font| families.iter().any(|family| font.uses_family(family)));
		self.dirty.append(&mut invalidation.rects);
		// A family list may now resolve to another face for painting too.
		self.painter.forget_fonts();
		self.full_repaint |= invalidation.fonts > 0;
		invalidation.blocks as u32
	}

	/// Lay out text in `families` but draw it invisibly, as browsers do
	/// during a `font-display` block period. Pass an empty list once the
	/// faces load or the period ends.
	#[wasm_bindgen(js_name = hideFamilies)]
	pub fn hide_families(&mut self, families: Vec<String>) {
		if self.painter.hide_families(families) {
			self.full_repaint = true;
		}
	}

	/// Snap text baselines and rectangle edges to the grid the browser
	/// snaps DOM content to: `"device"` pixels, or `"css"` pixels for
	/// Chromium laying out in CSS units on a high-density screen.
	#[wasm_bindgen(js_name = setPixelGrid)]
	pub fn set_pixel_grid(&mut self, grid: &str) -> JsResult<()> {
		let grid = match grid {
			"device" => PixelGrid::Device,
			"css" => PixelGrid::Css,
			other => {
				return Err(invalid_args(&format!(
					"the pixel grid must be \"device\" or \"css\", not {other:?}"
				)));
			}
		};

		if self.painter.set_grid(grid) {
			self.full_repaint = true;
		}

		Ok(())
	}

	/// Snap border widths down to `scale` device pixels per CSS pixel, as
	/// this browser draws them, or not at all when `scale` is not positive.
	#[wasm_bindgen(js_name = setBorderScale)]
	pub fn set_border_scale(&mut self, scale: f32) {
		let scale = (scale > 0.0).then_some(scale);

		if scale.map(f32::to_bits) != self.layout.options().border_scale.map(f32::to_bits) {
			self.layout.set_border_scale(self.doc.document(), scale);
			self.full_repaint = true;
		}
	}

	/// Forget every measurement, as when the device pixel ratio changes:
	/// browsers round font metrics to device pixels, so line positions can
	/// move. Blocks lay out again as they come into view.
	#[wasm_bindgen(js_name = forgetMeasurements)]
	pub fn forget_measurements(&mut self) {
		self.layout.forget_measurements();
		self.painter.forget_fonts();
		self.full_repaint = true;
	}

	/// The CSS `font` shorthand of every font the layout has used, indexed
	/// by font id.
	pub fn fonts(&self) -> Vec<String> {
		self.layout
			.fonts()
			.fonts()
			.iter()
			.map(canvist_layout::FontSpec::css)
			.collect()
	}

	/// Run the command an extension registered as `name`, with JSON `args`
	/// (`null` for none). Returns whether it applied.
	pub fn run(&mut self, name: &str, args: &str) -> JsResult<bool> {
		let args = parse_args(args)?;
		let command = self.doc.command_named(name, &args).map_err(headless)?;

		#[cfg(feature = "collab")]
		if let Some(redo) = self.doc.shared_history_command(name) {
			return self.history(redo);
		}

		self.edit(|editor, env| editor.run(&command, env))
	}

	/// Whether the named command would apply. Nothing changes.
	pub fn can(&mut self, name: &str, args: &str) -> JsResult<bool> {
		let args = parse_args(args)?;
		self.doc.can_named(name, &args).map_err(headless)
	}

	/// Answer the query an extension registered as `name`, as JSON.
	pub fn query(&self, name: &str, args: &str) -> JsResult<String> {
		let args = parse_args(args)?;
		self.doc
			.query(name, &args)
			.map(|answer| answer.to_string())
			.map_err(headless)
	}

	/// The anchor and head as JSON positions, or `null` for a block
	/// selection.
	pub fn selection(&self) -> String {
		self.doc.selected_range().map_or_else(
			|| "null".to_owned(),
			|(anchor, head)| {
				format!(
					r#"{{"anchor":{},"head":{}}}"#,
					position_json(&anchor),
					position_json(&head)
				)
			},
		)
	}

	/// Select from (`anchor_path`, `anchor_offset`) to (`head_path`,
	/// `head_offset`). Offsets count UTF-16 code units.
	pub fn select(
		&mut self,
		anchor_path: &[u32],
		anchor_offset: u32,
		head_path: &[u32],
		head_offset: u32,
	) -> JsResult<()> {
		let anchor = self
			.doc
			.point(&Position::new(anchor_path.to_vec(), anchor_offset))
			.map_err(headless)?;
		let head = self
			.doc
			.point(&Position::new(head_path.to_vec(), head_offset))
			.map_err(headless)?;
		self.edit(|editor, env| editor.select(Selection::text(anchor, head), env))
			.map(drop)
	}

	/// Replace the selection with `text`, running input rules as typing
	/// does. Returns whether anything changed.
	#[wasm_bindgen(js_name = insertText)]
	pub fn insert_text(&mut self, text: &str) -> JsResult<bool> {
		self.edit(|editor, env| editor.insert_text(text, env))
	}

	/// Undo the last change. A shared document undoes only this editor's
	/// own changes.
	pub fn undo(&mut self) -> JsResult<bool> {
		self.history(false)
	}

	/// Redo the last undone change.
	pub fn redo(&mut self) -> JsResult<bool> {
		self.history(true)
	}

	/// The caret's rectangle in document coordinates as `[x, y, width,
	/// height]`, or an empty array when the selection is not a caret. Hosts
	/// place IME windows and scroll the caret into view with it.
	#[wasm_bindgen(js_name = caretRect)]
	pub fn caret_rect(&self) -> Vec<f32> {
		rect_array(self.caret().and_then(|caret| self.layout.caret_rect(caret)))
	}

	/// The smallest rectangle around the selection in document coordinates,
	/// as `[x, y, width, height]`: the caret's rectangle for a caret, and
	/// empty while the selection is not laid out. Hosts anchor menus to it.
	#[wasm_bindgen(js_name = selectionBounds)]
	pub fn selection_bounds(&self) -> Vec<f32> {
		let bounds = match self.caret() {
			Some(caret) => self.layout.caret_rect(caret),
			None => self.layout.selection_bounds(&self.doc.selection()),
		};

		rect_array(bounds)
	}

	/// Whether the document is a single empty text block, as a new one is.
	/// Hosts show a placeholder then.
	#[wasm_bindgen(js_name = isEmpty)]
	pub fn is_empty(&self) -> bool {
		let document = self.doc.document();

		match document.children(None) {
			[block] => {
				document
					.inline(*block)
					.is_some_and(|text| text.as_str().is_empty())
			}
			_ => false,
		}
	}

	/// Body text's CSS `font` shorthand and colour, as `[font, color]`, for
	/// text the host draws over the canvas in the document's style, such as
	/// a placeholder.
	#[wasm_bindgen(js_name = bodyStyle)]
	pub fn body_style(&self) -> Vec<String> {
		let base = self.options.theme.base();
		vec![base.font.css(), base.color.to_string()]
	}

	/// The whole document laid out, as JSON: block rectangles, caret stops,
	/// and text runs (see the parity tests). Lays out every block first.
	pub fn geometry(&mut self) -> JsResult<String> {
		let list = self.full_display_list()?;
		Ok(geometry::geometry(self.doc.document(), &self.layout, &list).to_string())
	}

	/// The whole document as an SVG image painted from the same display
	/// list as the canvas. Lays out every block first.
	pub fn svg(&mut self) -> JsResult<String> {
		let list = self.full_display_list()?;
		Ok(canvist_layout::to_svg(&list))
	}

	/// Measurement and cache statistics as JSON: cache hits and misses,
	/// cached bytes per font, and how many segments and batches the browser
	/// measured. The host derives the hit rate, so no float is formatted here
	/// (see `canvist_layout::css_number`).
	pub fn stats(&self) -> String {
		let stats = self.layout.measure_stats();
		let fonts: Vec<String> = stats
			.fonts
			.iter()
			.enumerate()
			.map(|(index, font)| {
				format!(
					r#"{{"font":{},"segments":{},"bytes":{}}}"#,
					Json::from(self.font_css(index)),
					font.segments,
					font.bytes
				)
			})
			.collect();

		format!(
			r#"{{"hits":{},"misses":{},"bytes":{},"measuredSegments":{},"measureCalls":{},"fonts":[{}]}}"#,
			stats.hits,
			stats.misses,
			stats.bytes(),
			self.measurer.segments(),
			self.measurer.calls(),
			fonts.join(",")
		)
	}
}

impl WebEditor {
	/// Swap in `doc`, laid out from scratch on the next frame.
	fn replace(&mut self, doc: Doc) {
		self.doc = doc;
		// Keep what the host told the old layout about the screen.
		let LayoutOptions {
			width,
			border_scale,
			..
		} = *self.layout.options();
		let mut options = layout_options(&self.options, width);
		options.border_scale = border_scale;
		self.layout = DocumentLayout::new(self.doc.document(), self.options.theme.clone(), options);
		self.painted = None;
		self.full_repaint = true;
	}

	/// Undo or redo one step through the document's history, which is the
	/// shared one for a shared document.
	fn history(&mut self, redo: bool) -> JsResult<bool> {
		let update = self.doc.history_step(redo).map_err(headless)?;

		Ok(update.is_some_and(|update| {
			self.lay_out(&update.commit);
			update.doc_changed() || update.selection_changed()
		}))
	}

	/// Let layout account for `commit`.
	fn lay_out(&mut self, commit: &Commit) {
		let damage = self.layout.apply(self.doc.document(), commit);

		if damage.from_y.is_some() {
			self.full_repaint = true;
		}

		self.caret_on = true;
	}

	/// Make a change through the editor with the wall clock and this
	/// layout's line navigator, then let layout account for it.
	fn edit(
		&mut self,
		change: impl FnOnce(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> JsResult<bool> {
		Ok(self
			.edit_update(change)?
			.is_some_and(|update| update.doc_changed() || update.selection_changed()))
	}

	/// [`WebEditor::edit`], returning the update it applied.
	fn edit_update(
		&mut self,
		change: impl FnOnce(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> JsResult<Option<Update>> {
		let mut change = Some(change);

		self.apply_change(&mut |editor, env| {
			change.take().map_or(Ok(None), |change| change(editor, env))
		})
	}

	/// [`WebEditor::edit_update`] without the generic closure, so the many
	/// input methods share one copy of it in the WebAssembly module.
	fn apply_change(
		&mut self,
		change: &mut dyn FnMut(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> JsResult<Option<Update>> {
		let now = Duration::from_secs_f64(js_sys::Date::now() / 1000.0);
		let mut lines = HostLines::new(&mut self.layout, &mut self.measurer);
		let update = self.doc.edit(now, &mut lines, change).map_err(headless)?;

		let Some(update) = update else {
			return Ok(None);
		};

		self.lay_out(&update.commit);
		Ok(Some(update))
	}

	fn full_display_list(&mut self) -> JsResult<DisplayList> {
		self.layout
			.layout_all(self.doc.document(), &mut self.measurer)
			.map_err(|error| render(&error))?;
		Ok(self
			.layout
			.display_list(&DisplayOptions::viewport(0.0, self.layout.height())))
	}

	fn caret(&self) -> Option<Caret> {
		match self.doc.selection() {
			Selection::Text { anchor, head } if anchor == head => {
				let affinity = self.doc.state().affinity().into();
				Some(Caret::with_affinity(head, affinity))
			}
			_ => None,
		}
	}

	/// What to draw in the viewport ending at `bottom`: a collapsed selection
	/// is a caret, drawn only while the editor has focus and the caret is in
	/// the visible half of its blink; any other selection is highlighted.
	fn display_options(&self, bottom: f32) -> DisplayOptions {
		let options = DisplayOptions::viewport(self.top, bottom);

		let Some(caret) = self.caret() else {
			return options.selection(self.doc.selection());
		};

		if self.focused && self.caret_on {
			options.caret(caret)
		} else {
			options
		}
	}

	/// The rectangles that differ between the canvas and `list`, plus damage
	/// the diff cannot see. Empty when the whole canvas repaints anyway.
	fn damage(&mut self, list: &DisplayList, full: bool) -> Vec<Rect> {
		let Some(previous) = self.painted.as_ref().filter(|_| !full) else {
			return Vec::new();
		};

		let mut damage = list.damage(previous);
		damage.append(&mut self.dirty);

		damage
	}

	/// Paint `region` of `list` at the current scroll position.
	fn paint(&mut self, list: &DisplayList, region: &Region<'_>) {
		let measurer = &self.measurer;
		let font = |id| measurer.paint_font(id);
		let background = self.background();

		self.painter
			.paint(list, self.top, background, region, &font);
	}

	fn background(&self) -> canvist_layout::Color {
		self.options.theme.chrome().background
	}

	fn font_css(&self, index: usize) -> Option<String> {
		self.layout
			.fonts()
			.fonts()
			.get(index)
			.map(canvist_layout::FontSpec::css)
	}
}

/// A rectangle as `[x, y, width, height]`, or empty for `None`.
fn rect_array(rect: Option<Rect>) -> Vec<f32> {
	rect.map_or_else(Vec::new, |rect| {
		vec![rect.x, rect.y, rect.width, rect.height]
	})
}

/// A position as JSON: `{"path":[…],"offset":n}`.
fn position_json(position: &Position) -> String {
	let path: Vec<String> = position.path.iter().map(u32::to_string).collect();
	format!(
		r#"{{"path":[{}],"offset":{}}}"#,
		path.join(","),
		position.offset
	)
}

fn layout_options(options: &Options, width: f32) -> LayoutOptions {
	let layout = LayoutOptions::new(width).overscan(options.overscan);

	match options.breaks.override_fn() {
		Some(breaks) => layout.break_override(breaks),
		None => layout,
	}
}

fn parse_args(args: &str) -> JsResult<Json> {
	serde_json::from_str(args).map_err(|error| invalid_args(&error.to_string()))
}
