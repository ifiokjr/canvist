//! The scene walker: one page of a scene, culled to the viewport, as
//! [`Painter`] calls in paint order.
//!
//! Subtrees outside the viewport are skipped by their extent. Nodes smaller
//! than a pixel on screen are skipped; nodes smaller than [`LOD_PIXELS`]
//! draw as a box in their main colour; text smaller than [`GREEK_PIXELS`]
//! draws as bars. In [`Quality::Interactive`] (while the camera moves)
//! shadows are skipped too.

use std::borrow::Cow;

use canvist_canvas::Camera;
use canvist_canvas::NodeGeometry;
use canvist_canvas::SceneIndex;
use canvist_canvas::TextLayouts;
use canvist_layout::DisplayItem;
use canvist_scene::EffectKind;
use canvist_scene::Node;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::Paint;
use canvist_scene::PaintKind;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::Shape;
use canvist_scene::StrokeAlign;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Point;
use kurbo::Rect;
use kurbo::Shape as _;
use kurbo::Size;
use peniko::Brush;

use crate::Assets;
use crate::convert;
use crate::painter::BoxShadow;
use crate::painter::Draw;
use crate::painter::Image;
use crate::painter::Layer;
use crate::painter::Painter;
use crate::painter::Text;

/// Nodes smaller than this many screen pixels are not drawn.
pub const MIN_PIXELS: f64 = 0.5;
/// Nodes smaller than this many screen pixels draw as a box in their main
/// colour.
pub const LOD_PIXELS: f64 = 3.0;
/// Text whose font is smaller than this many screen pixels draws as bars.
pub const GREEK_PIXELS: f64 = 4.0;
/// How deeply instances may nest before the walker stops, so a component
/// that contains an instance of itself cannot recurse forever.
const MAX_INSTANCE_DEPTH: usize = 8;
/// The opacity of greeked text bars.
const GREEK_ALPHA: f32 = 0.35;
/// A greeked bar's height, as a fraction of the font size: about an
/// x-height.
const GREEK_HEIGHT: f64 = 0.3;
/// How far above the baseline a greeked bar sits, in bar heights.
const GREEK_RISE: f64 = 1.5;
/// The light grey shown where an image has not loaded.
const PLACEHOLDER: peniko::Color = peniko::Color::from_rgba8(229, 229, 229, 255);

/// How much detail to draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
	/// Everything.
	#[default]
	Full,
	/// Skip shadows, for frames drawn while the camera moves.
	Interactive,
}

/// What to draw: a camera over a viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
	/// The camera.
	pub camera: Camera,
	/// The viewport size in screen pixels.
	pub viewport: Size,
	/// How much detail to draw.
	pub quality: Quality,
}

/// The scene and everything derived from it that painting reads.
#[derive(Clone, Copy, Debug)]
pub struct Sources<'a> {
	/// The scene.
	pub scene: &'a Scene,
	/// Its index.
	pub index: &'a SceneIndex,
	/// Its text frame layouts.
	pub texts: &'a TextLayouts,
	/// Decoded images by asset id.
	pub assets: &'a Assets,
}

/// Paint `page` as `view` sees it.
pub fn paint_page<P: Painter + ?Sized>(
	painter: &mut P,
	sources: &Sources<'_>,
	page: NodeId,
	view: &View,
) {
	let visible = view.camera.visible(view.viewport);
	let mut walker = Walker {
		painter,
		sources,
		view,
		visible,
		to_screen: view.camera.world_to_screen(),
		depth: 0,
	};

	walker.background(page);

	for &child in sources.scene.children(page) {
		walker.walk(child, &Placement::default());
	}
}

/// Paint the subtrees of `nodes` with `world_to_target` mapping world
/// coordinates to the output, with no culling: for exports.
pub fn paint_nodes<P: Painter + ?Sized>(
	painter: &mut P,
	sources: &Sources<'_>,
	nodes: &[NodeId],
	world_to_target: Affine,
) {
	let view = View {
		camera: Camera::default(),
		viewport: Size::new(f64::MAX, f64::MAX),
		quality: Quality::Full,
	};
	let mut walker = Walker {
		painter,
		sources,
		view: &view,
		visible: Rect::new(f64::MIN, f64::MIN, f64::MAX, f64::MAX),
		to_screen: world_to_target,
		depth: 0,
	};

	for &node in nodes {
		walker.walk(node, &Placement::default());
	}
}

/// Where a subtree is drawn from: an instance places its component's nodes
/// elsewhere and overrides their props.
#[derive(Clone, Default)]
struct Placement<'a> {
	/// Applied before each node's world transform.
	extra: Option<Affine>,
	/// The instance whose overrides apply.
	instance: Option<&'a Props>,
}

/// A shape being painted: its outline, its box size, and where it goes.
struct Surface<'a> {
	outline: &'a BezPath,
	size: kurbo::Vec2,
	transform: Affine,
}

/// What a text frame's items share.
struct TextStyle<'a> {
	fonts: &'a [canvist_layout::FontSpec],
	/// The frame's fill, which colours all of its text.
	fill: Option<peniko::Color>,
	/// Screen pixels per frame unit, to decide when text is too small.
	scale: f64,
	transform: Affine,
}

struct Walker<'a, P: ?Sized> {
	painter: &'a mut P,
	sources: &'a Sources<'a>,
	view: &'a View,
	visible: Rect,
	to_screen: Affine,
	depth: usize,
}

impl<P: Painter + ?Sized> Walker<'_, P> {
	/// Fill the viewport with the page's fills.
	fn background(&mut self, page: NodeId) {
		let Some(node) = self.sources.scene.node(page) else {
			return;
		};
		let area = self.visible.to_path(geometry_tolerance());
		let origin = Affine::translate(self.visible.origin().to_vec2());

		for paint in node.props().fills.iter().filter(|paint| paint.visible) {
			if let Some((brush, unit)) = convert::brush(paint, self.visible.size().to_vec2()) {
				let brush_transform = unit.map(|unit| origin * unit);
				self.painter.fill(
					&draw(&area, self.to_screen, &brush, brush_transform),
					peniko::Fill::NonZero,
				);
			}
		}
	}

	fn walk(&mut self, id: NodeId, placement: &Placement<'_>) {
		let scene = self.sources.scene;
		let (Some(node), Some(geometry)) = (scene.node(id), self.sources.index.get(id)) else {
			return;
		};
		let props = effective_props(node, placement.instance);

		if !props.visible {
			return;
		}

		let world = placement
			.extra
			.map_or(geometry.world, |extra| extra * geometry.world);
		let extent = placement.extra.map_or(geometry.extent, |extra| {
			transformed_bounds(extra, geometry.extent)
		});

		if !extent.overlaps(self.visible) && !extent.contains(self.visible.origin()) {
			return;
		}

		let pixels = extent.width().max(extent.height()) * self.view.camera.zoom;

		if pixels < MIN_PIXELS {
			return;
		}

		let transform = self.to_screen * world;
		let kind = scene.kind(id).unwrap_or(NodeKind::Group);
		let leaf = scene.children(id).is_empty();

		if pixels < LOD_PIXELS && leaf {
			self.summary(geometry, &props, transform);

			return;
		}

		let layered = self.open_layer(&props, geometry, transform, leaf);
		self.body(id, kind, &props, geometry, transform);
		self.children(id, kind, &props, world, placement);

		if layered {
			self.painter.pop_layer();
		}
	}

	/// Draw a node too small to detail as its box in its main colour.
	fn summary(&mut self, geometry: &NodeGeometry, props: &Props, transform: Affine) {
		let Some(color) = convert::summary_color(&props.fills) else {
			return;
		};
		let size = geometry.size;
		let rect =
			Rect::new(0.0, 0.0, size.x.max(1.0), size.y.max(1.0)).to_path(geometry_tolerance());
		self.painter.fill(
			&draw(&rect, transform, &Brush::Solid(color), None),
			peniko::Fill::NonZero,
		);
	}

	/// Start a layer when the node fades, blends, or clips its children.
	fn open_layer(
		&mut self,
		props: &Props,
		geometry: &NodeGeometry,
		transform: Affine,
		leaf: bool,
	) -> bool {
		let clips = props.clip && !leaf;
		let needed =
			props.opacity < 1.0 || props.blend != canvist_scene::BlendMode::Normal || clips;

		if !needed {
			return false;
		}

		let clip = clips.then_some(geometry.outline.as_deref()).flatten();
		self.painter.push_layer(&Layer {
			clip,
			transform,
			blend: convert::blend(props.blend),
			alpha: props.opacity as f32,
		});
		true
	}

	fn body(
		&mut self,
		id: NodeId,
		kind: NodeKind,
		props: &Props,
		geometry: &NodeGeometry,
		transform: Affine,
	) {
		if self.view.quality == Quality::Full {
			self.shadows(props, geometry, transform);
		}

		let Some(outline) = geometry.outline.as_deref() else {
			return;
		};
		let surface = Surface {
			outline,
			size: geometry.size,
			transform,
		};

		// Outside strokes go behind the fill, the others on top of it.
		self.strokes(&surface, props, StrokeAlign::Outside);
		self.fills(&surface, props, kind);
		self.strokes(&surface, props, StrokeAlign::Center);
		self.strokes(&surface, props, StrokeAlign::Inside);

		if kind == NodeKind::Text {
			self.text(id, props, transform);
		}
	}

	fn shadows(&mut self, props: &Props, geometry: &NodeGeometry, transform: Affine) {
		let radius = props.radius.iter().copied().fold(0.0, f64::max);

		for effect in props.effects.iter().filter(|effect| effect.visible) {
			let EffectKind::DropShadow(shadow) = effect.kind else {
				continue;
			};
			let rect = Rect::from_origin_size(Point::ORIGIN, geometry.size.to_size())
				.inflate(shadow.spread, shadow.spread)
				+ shadow.offset;

			self.painter.shadow(&BoxShadow {
				rect,
				radius,
				transform,
				color: convert::color(shadow.color),
				blur: shadow.blur,
			});
		}
	}

	/// Paint each visible fill. Text frames' fills colour their text instead.
	fn fills(&mut self, surface: &Surface<'_>, props: &Props, kind: NodeKind) {
		if kind == NodeKind::Text {
			return;
		}

		let rule = convert::fill_rule(crate::fill_rule_of(&props.shape));

		for paint in props.fills.iter().filter(|paint| paint.visible) {
			match &paint.kind {
				PaintKind::Image { asset, fit } => {
					self.image_fill(surface, asset, *fit, paint.opacity);
				}
				_ => self.blended(paint, |painter| solid_fill(painter, surface, paint, rule)),
			}
		}
	}

	/// Run `paint_with` inside a blend layer when the paint blends.
	fn blended(&mut self, paint: &Paint, paint_with: impl FnOnce(&mut P)) {
		let layered = paint.blend != canvist_scene::BlendMode::Normal;

		if layered {
			self.painter.push_layer(&Layer {
				clip: None,
				transform: Affine::IDENTITY,
				blend: convert::blend(paint.blend),
				alpha: 1.0,
			});
		}

		paint_with(self.painter);

		if layered {
			self.painter.pop_layer();
		}
	}

	/// An image paint, clipped to the outline and placed by its fit.
	fn image_fill(
		&mut self,
		surface: &Surface<'_>,
		asset: &str,
		fit: canvist_scene::ImageFit,
		opacity: f64,
	) {
		let Some(data) = self.sources.assets.get(asset) else {
			self.placeholder(surface);

			return;
		};
		let image_size = Size::new(f64::from(data.width), f64::from(data.height));
		let placed = crate::fit::image_transform(fit, image_size, surface.size.to_size());

		self.painter.push_layer(&clip_layer(surface));
		self.painter.image(&Image {
			asset,
			data,
			transform: surface.transform * placed,
			alpha: opacity as f32,
		});
		self.painter.pop_layer();
	}

	/// A light grey box where an image that is not loaded goes.
	fn placeholder(&mut self, surface: &Surface<'_>) {
		let grey = Brush::Solid(PLACEHOLDER);
		self.painter.fill(
			&draw(surface.outline, surface.transform, &grey, None),
			peniko::Fill::NonZero,
		);
	}

	/// Paint the strokes aligned with `align`. Inside and outside strokes
	/// draw twice as wide: inside ones clipped to the outline, outside ones
	/// behind the fill.
	fn strokes(&mut self, surface: &Surface<'_>, props: &Props, align: StrokeAlign) {
		if props.stroke.align != align || props.stroke.width <= 0.0 {
			return;
		}

		let scale = if align == StrokeAlign::Center {
			1.0
		} else {
			2.0
		};

		let style = convert::stroke(&props.stroke, scale);
		let clipped = align == StrokeAlign::Inside;

		if clipped {
			self.painter.push_layer(&clip_layer(surface));
		}

		for paint in props.strokes.iter().filter(|paint| paint.visible) {
			let Some((brush, brush_transform)) = convert::brush(paint, surface.size) else {
				continue;
			};
			self.painter.stroke(
				&draw(surface.outline, surface.transform, &brush, brush_transform),
				&style,
			);
		}

		if clipped {
			self.painter.pop_layer();
		}
	}

	/// Draw a text frame's laid-out runs, decorations, and backgrounds.
	fn text(&mut self, id: NodeId, props: &Props, transform: Affine) {
		let Some(frame) = self.sources.texts.get(id) else {
			return;
		};
		let style = TextStyle {
			fonts: &frame.display.fonts,
			fill: convert::summary_color(&props.fills),
			scale: transform.as_coeffs()[0].hypot(transform.as_coeffs()[1]),
			transform,
		};

		for item in &frame.display.items {
			self.display_item(item, &style);
		}
	}

	fn display_item(&mut self, item: &DisplayItem, style: &TextStyle<'_>) {
		match item {
			DisplayItem::Text(run) => self.run(run, style),
			DisplayItem::Rect {
				rect,
				color,
				radius,
				..
			} => {
				let shape = kurbo::RoundedRect::from_rect(layout_rect(*rect), f64::from(*radius))
					.to_path(geometry_tolerance());
				let brush = Brush::Solid(convert::layout_color(*color));
				self.painter.fill(
					&draw(&shape, style.transform, &brush, None),
					peniko::Fill::NonZero,
				);
			}
			DisplayItem::Decoration {
				x,
				y,
				width,
				thickness,
				color,
				..
			} => {
				let half = f64::from(*thickness) / 2.0;
				let line = Rect::new(
					f64::from(*x),
					f64::from(*y) - half,
					f64::from(*x + *width),
					f64::from(*y) + half,
				);
				let brush =
					Brush::Solid(style.fill.unwrap_or_else(|| convert::layout_color(*color)));
				self.painter.fill(
					&draw(
						&line.to_path(geometry_tolerance()),
						style.transform,
						&brush,
						None,
					),
					peniko::Fill::NonZero,
				);
			}
			// Node views are host-rendered; the canvas leaves their slots empty.
			DisplayItem::Slot { .. } => {}
		}
	}

	/// One text run, or a bar where it is too small to read.
	fn run(&mut self, run: &canvist_layout::TextRun, style: &TextStyle<'_>) {
		let Some(font) = style.fonts.get(run.font.index()) else {
			return;
		};
		let color = style
			.fill
			.unwrap_or_else(|| convert::layout_color(run.color));

		if f64::from(font.size) * style.scale >= GREEK_PIXELS {
			self.painter.text(&Text {
				run,
				font,
				transform: style.transform,
				color,
			});

			return;
		}

		let height = f64::from(font.size) * GREEK_HEIGHT;
		let top = f64::from(run.baseline) - height * GREEK_RISE;
		let bar = Rect::new(
			f64::from(run.x),
			top,
			f64::from(run.x + run.width),
			top + height,
		);
		let brush = Brush::Solid(color.multiply_alpha(GREEK_ALPHA));
		self.painter.fill(
			&draw(
				&bar.to_path(geometry_tolerance()),
				style.transform,
				&brush,
				None,
			),
			peniko::Fill::NonZero,
		);
	}

	fn children(
		&mut self,
		id: NodeId,
		kind: NodeKind,
		props: &Props,
		world: Affine,
		placement: &Placement<'_>,
	) {
		match kind {
			// A boolean's children draw only as its outline.
			NodeKind::Boolean => {}
			NodeKind::Instance => self.instance(props, world),
			_ => self.walk_all(self.sources.scene.children(id), placement),
		}
	}

	fn walk_all(&mut self, ids: &[NodeId], placement: &Placement<'_>) {
		for &child in ids {
			self.walk(child, placement);
		}
	}

	/// Draw an instance's component in the instance's place, with its
	/// overrides.
	fn instance(&mut self, props: &Props, world: Affine) {
		let Shape::Instance { component } = props.shape else {
			return;
		};
		let Some(source) = self.sources.index.get(component) else {
			return;
		};

		if self.depth >= MAX_INSTANCE_DEPTH {
			return;
		}

		let placement = Placement {
			extra: Some(world * source.world.inverse()),
			instance: Some(props),
		};
		self.depth += 1;
		self.walk(component, &placement);
		self.depth -= 1;
	}
}

/// Fill `surface` with a non-image paint.
fn solid_fill<P: Painter + ?Sized>(
	painter: &mut P,
	surface: &Surface<'_>,
	paint: &Paint,
	rule: peniko::Fill,
) {
	if let Some((brush, brush_transform)) = convert::brush(paint, surface.size) {
		painter.fill(
			&draw(surface.outline, surface.transform, &brush, brush_transform),
			rule,
		);
	}
}

/// A plain layer clipped to `surface`'s outline.
fn clip_layer<'a>(surface: &Surface<'a>) -> Layer<'a> {
	Layer {
		clip: Some(surface.outline),
		transform: surface.transform,
		blend: peniko::BlendMode::default(),
		alpha: 1.0,
	}
}

/// A node's props with an instance's overrides for it applied.
fn effective_props<'a>(node: &'a Node, instance: Option<&Props>) -> Cow<'a, Props> {
	let Some(instance) = instance else {
		return Cow::Borrowed(node.props());
	};
	let overrides: Vec<_> = instance
		.overrides
		.iter()
		.filter(|((target, _), _)| *target == node.id())
		.map(|(_, prop)| prop.clone())
		.collect();

	if overrides.is_empty() {
		return Cow::Borrowed(node.props());
	}

	let mut props = node.props().clone();

	for prop in overrides {
		props.set(prop);
	}

	Cow::Owned(props)
}

fn draw<'a>(
	path: &'a BezPath,
	transform: Affine,
	brush: &'a Brush,
	brush_transform: Option<Affine>,
) -> Draw<'a> {
	Draw {
		path,
		transform,
		brush,
		brush_transform,
	}
}

fn layout_rect(rect: canvist_layout::Rect) -> Rect {
	Rect::new(
		f64::from(rect.x),
		f64::from(rect.y),
		f64::from(rect.x + rect.width),
		f64::from(rect.y + rect.height),
	)
}

/// The bounding box of `rect` after `transform`.
fn transformed_bounds(transform: Affine, rect: Rect) -> Rect {
	let corners = [
		Point::new(rect.x0, rect.y0),
		Point::new(rect.x1, rect.y0),
		Point::new(rect.x1, rect.y1),
		Point::new(rect.x0, rect.y1),
	]
	.map(|corner| transform * corner);

	corners.iter().skip(1).fold(
		Rect::from_points(corners[0], corners[0]),
		|bounds, &corner| bounds.union_pt(corner),
	)
}

fn geometry_tolerance() -> f64 {
	canvist_canvas::geometry::TOLERANCE
}
