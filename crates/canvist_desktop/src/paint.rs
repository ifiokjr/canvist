//! Painting: the last frame's display list through any `canvist_paint`
//! painter, plus what the display list leaves to the host (task
//! checkboxes) and the composition underline.

use canvist_layout::Color;
use canvist_layout::DisplayItem;
use canvist_layout::Rect;
use canvist_layout::SlotKind;
use canvist_paint::CpuPainter;
use canvist_paint::DisplayStyle;
use canvist_paint::Draw;
use canvist_paint::Layer;
use canvist_paint::Painter;
use canvist_paint::convert::layout_color;
use canvist_paint::peniko::BlendMode;
use canvist_paint::peniko::Brush;
use canvist_paint::peniko::Fill;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Shape as _;
use kurbo::Stroke;

use crate::DesktopEditor;
use crate::DesktopResult;

/// Flattening tolerance for shapes, in logical pixels.
const TOLERANCE: f64 = 0.1;
/// The composition underline's thickness, in logical pixels.
const UNDERLINE: f32 = 1.5;
/// A checkbox's corner radius and stroke, as fractions of its side.
const CHECKBOX_RADIUS: f64 = 0.2;
const CHECKBOX_STROKE: f64 = 0.1;

impl<Handles: Copy> DesktopEditor<Handles> {
	/// Paint the last [`DesktopEditor::frame`] into `painter`. `to_screen`
	/// maps the editor's logical coordinates to the painter's pixels: scale
	/// by the viewport's scale, and translate to wherever the host placed the
	/// editor in its window.
	pub fn paint<P: Painter + ?Sized>(&self, painter: &mut P, to_screen: Affine) {
		let viewport = self.viewport;
		let area = kurbo::Rect::new(
			0.0,
			0.0,
			f64::from(viewport.width),
			f64::from(viewport.height),
		)
		.to_path(TOLERANCE);
		let background = Brush::Solid(layout_color(self.options.theme.chrome().background));

		painter.fill(&draw(&area, to_screen, &background), Fill::NonZero);
		painter.push_layer(&Layer {
			clip: Some(&area),
			transform: to_screen,
			blend: BlendMode::default(),
			alpha: 1.0,
		});

		let (left, top) = self.to_view(0.0, 0.0);
		let document = to_screen * Affine::translate((f64::from(left), f64::from(top)));
		canvist_paint::paint_display_list(painter, &self.list, &DisplayStyle::new(document));
		self.paint_slots(painter, document);
		self.paint_composition(painter, document);

		painter.pop_layer();
	}

	/// The last frame as straight-alpha RGBA8 pixels at the viewport's scale,
	/// rasterised on the CPU: screenshots, thumbnails, and pixel tests.
	pub fn render_rgba8(&self) -> DesktopResult<(u32, u32, Vec<u8>)> {
		let (width, height) = self.pixel_size();
		let mut painter = self.cpu_painter(width, height)?;
		Ok((width, height, painter.render_rgba8()))
	}

	/// The last frame as a PNG file, at the viewport's scale.
	pub fn render_png(&self) -> DesktopResult<Vec<u8>> {
		let (width, height) = self.pixel_size();
		let mut painter = self.cpu_painter(width, height)?;
		Ok(painter.render_png()?)
	}

	/// The viewport in physical pixels.
	pub fn pixel_size(&self) -> (u32, u32) {
		let viewport = self.viewport;
		(
			(viewport.width * viewport.scale).round().max(1.0) as u32,
			(viewport.height * viewport.scale).round().max(1.0) as u32,
		)
	}

	fn cpu_painter(&self, width: u32, height: u32) -> DesktopResult<CpuPainter> {
		let mut painter = CpuPainter::new(width, height)?;
		*painter.shaper_mut() = self.fonts.shaper();
		self.paint(&mut painter, Affine::scale(f64::from(self.viewport.scale)));
		Ok(painter)
	}

	/// Draw the views the display list reserves slots for: task checkboxes.
	/// Images and embeds are left to the host.
	fn paint_slots<P: Painter + ?Sized>(&self, painter: &mut P, document: Affine) {
		let checkbox = Checkbox {
			accent: self.options.theme.chrome().caret,
			outline: self.options.theme.base().color,
		};

		for item in &self.list.items {
			if let DisplayItem::Slot {
				kind: SlotKind::Checkbox { checked },
				rect,
				..
			} = item
			{
				checkbox.paint(painter, document, *rect, *checked);
			}
		}
	}

	/// Underline the text an input method is composing.
	fn paint_composition<P: Painter + ?Sized>(&self, painter: &mut P, document: Affine) {
		let color = Brush::Solid(layout_color(self.options.theme.base().color));

		for rect in self.composition_rects() {
			let line = kurbo::Rect::new(
				f64::from(rect.x),
				f64::from(rect.y + rect.height - UNDERLINE),
				f64::from(rect.x + rect.width),
				f64::from(rect.y + rect.height),
			)
			.to_path(TOLERANCE);
			painter.fill(&draw(&line, document, &color), Fill::NonZero);
		}
	}
}

/// How a task checkbox looks: a rounded box outlined in `outline`, or
/// filled with `accent` and ticked once checked.
struct Checkbox {
	accent: Color,
	outline: Color,
}

/// The tick's three points, as fractions of the box's side.
const TICK: [(f64, f64); 3] = [(0.25, 0.52), (0.43, 0.7), (0.76, 0.32)];
/// The tick's stroke, as a multiple of the box's.
const TICK_WEIGHT: f64 = 1.4;
/// The opacity of an unchecked box's outline.
const UNCHECKED_ALPHA: f32 = 0.6;

impl Checkbox {
	fn paint<P: Painter + ?Sized>(
		&self,
		painter: &mut P,
		document: Affine,
		rect: Rect,
		checked: bool,
	) {
		let side = f64::from(rect.width.min(rect.height));
		let origin = kurbo::Point::new(f64::from(rect.x), f64::from(rect.y));
		let bounds = kurbo::Rect::from_origin_size(origin, (side, side));
		let shape =
			kurbo::RoundedRect::from_rect(bounds, side * CHECKBOX_RADIUS).to_path(TOLERANCE);
		let stroke = Stroke::new(side * CHECKBOX_STROKE);

		if !checked {
			let brush = Brush::Solid(layout_color(self.outline).multiply_alpha(UNCHECKED_ALPHA));
			painter.stroke(&draw(&shape, document, &brush), &stroke);
			return;
		}

		let fill = Brush::Solid(layout_color(self.accent));
		painter.fill(&draw(&shape, document, &fill), Fill::NonZero);

		let mut tick = BezPath::new();

		for (index, (x, y)) in TICK.into_iter().enumerate() {
			let point = (bounds.x0 + side * x, bounds.y0 + side * y);

			if index == 0 {
				tick.move_to(point);
			} else {
				tick.line_to(point);
			}
		}

		let white = Brush::Solid(canvist_paint::peniko::Color::WHITE);
		let weight = Stroke::new(side * CHECKBOX_STROKE * TICK_WEIGHT);
		painter.stroke(&draw(&tick, document, &white), &weight);
	}
}

fn draw<'a>(path: &'a BezPath, transform: Affine, brush: &'a Brush) -> Draw<'a> {
	Draw {
		path,
		transform,
		brush,
		brush_transform: None,
	}
}
