#![doc(
	html_logo_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_paint`
//!
//! Rendering for canvist's canvas. The scene walker turns one page of a
//! scene into [`Painter`] calls, culled to the viewport with level of
//! detail, and every backend implements [`Painter`]:
//!
//! - [`VelloPainter`] (feature `gpu`): vello on wgpu, for WebGPU in browsers
//!   and Metal, Vulkan, and DX12 natively;
//! - [`CpuPainter`] (feature `cpu`): `vello_cpu`, for PNG export, server
//!   thumbnails, and the browser fallback where WebGPU is missing;
//! - [`SvgPainter`]: SVG documents, with text kept as text;
//! - [`Recorder`]: the calls themselves, for tests.
//!
//! Text frames draw their `canvist_layout` display list. The GPU and CPU
//! backends shape runs with `parley` and the bundled font the canvas
//! measures with, so glyphs land where layout put them.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_canvas::Canvas;
//! use canvist_canvas::Shapes;
//! use canvist_canvas::TextLayouts;
//! use canvist_layout::FixedMeasurer;
//! use canvist_layout::Theme;
//! use canvist_paint::Assets;
//! use canvist_paint::Sources;
//! use canvist_paint::SvgPainter;
//! use canvist_paint::View;
//! use canvist_scene::Props;
//! use canvist_scene::Scene;
//! use kurbo::Size;
//! use kurbo::Vec2;
//!
//! let mut builder = Canvas::builder();
//! builder.add(Shapes);
//! let mut scene = Scene::new(builder.schema()?, 1)?;
//! let page = scene.pages()[0];
//! let mut transaction = scene.transaction();
//! transaction.insert(page, None, "ellipse", Props {
//! 	size: Vec2::new(80.0, 40.0),
//! 	fills: Arc::new([canvist_scene::Paint::solid(canvist_scene::Color::rgb(59, 44, 245))]),
//! 	..Props::default()
//! })?;
//! let _ = transaction.commit();
//! let canvas = builder.build(scene, TextLayouts::new(Arc::new(Theme::starter_kit()), FixedMeasurer::new()))?;
//! let state = canvas.state();
//! let assets = Assets::new();
//! let sources = Sources { scene: state.scene(), index: state.index(), texts: state.texts(), assets: &assets };
//! let view = View { camera: *state.camera(), viewport: Size::new(200.0, 100.0), quality: Default::default() };
//!
//! let mut svg = SvgPainter::new(view.viewport);
//! canvist_paint::paint_page(&mut svg, &sources, page, &view);
//! assert!(svg.finish().contains("fill=\"#3b2cf5ff\""));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
mod assets;
mod chrome;
pub mod convert;
#[cfg(feature = "cpu")]
mod cpu;
pub mod fit;
#[cfg(feature = "gpu")]
mod gpu;

mod painter;
mod record;
mod svg;
mod text;
mod walk;

pub use assets::Assets;
pub use chrome::ChromeStyle;
pub use chrome::paint_overlay;
#[cfg(feature = "cpu")]
pub use cpu::CpuError;
#[cfg(feature = "cpu")]
pub use cpu::CpuPainter;
#[cfg(feature = "gpu")]
pub use gpu::VelloPainter;
pub use painter::BoxShadow;
pub use painter::Draw;
pub use painter::Image;
pub use painter::Layer;
pub use painter::Painter;
pub use painter::Text;
pub use peniko;
pub use record::PaintOp;
pub use record::Recorder;
pub use svg::SvgPainter;
pub use text::BUNDLED_FONT;
pub use text::Glyph;
pub use text::ShapedRun;
pub use text::Shaper;
#[cfg(feature = "gpu")]
pub use vello;
pub use walk::GREEK_PIXELS;
pub use walk::LOD_PIXELS;
pub use walk::MIN_PIXELS;
pub use walk::Quality;
pub use walk::Sources;
pub use walk::View;
pub use walk::paint_nodes;
pub use walk::paint_page;

/// The fill rule a shape's outline uses.
pub(crate) fn fill_rule_of(shape: &canvist_scene::Shape) -> canvist_scene::FillRule {
	match shape {
		canvist_scene::Shape::Path { rule, .. } => *rule,
		_ => canvist_scene::FillRule::NonZero,
	}
}
