#![doc(
	html_logo_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_canvas`
//!
//! The headless interaction core of canvist's canvas. Everything a design
//! tool's canvas does between input and pixels, with no UI:
//!
//! - a [`Camera`] that pans, zooms about a point, coasts with [`Inertia`],
//!   and pinches;
//! - a [`SceneIndex`] of world transforms, sizes (auto-sized text, hugging
//!   frames, auto-layout), outlines, and an R-tree, updated per commit;
//! - exact [`hit_test`]s on fills, strokes, and curves, selection targets,
//!   and [`marquee`] selection;
//! - transform [`handles`] that resize, rotate, and skew, with constraints
//!   and size kept separate from transform;
//! - [`Snapper`] smart guides and ruler [`ticks`];
//! - [`Tool`]s as typed extensions, commands, and inspectors as data;
//! - an [`OverlayItem`] list: the chrome, as data, for the host to draw.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_canvas::Canvas;
//! use canvist_canvas::Input;
//! use canvist_canvas::PointerEvent;
//! use canvist_canvas::Phase;
//! use canvist_canvas::Shapes;
//! use canvist_canvas::TextLayouts;
//! use canvist_layout::FixedMeasurer;
//! use canvist_layout::Theme;
//! use canvist_scene::Scene;
//! use kurbo::Point;
//!
//! let mut builder = Canvas::builder();
//! let shapes = builder.add(Shapes);
//! let scene = Scene::new(builder.schema()?, 1)?;
//! let texts = TextLayouts::new(Arc::new(Theme::starter_kit()), FixedMeasurer::new());
//! let mut canvas = builder.build(scene, texts)?;
//!
//! // Drag out a rectangle with the rectangle tool.
//! canvas.set_tool(shapes.rectangle);
//! for (phase, x, y) in [(Phase::Down, 10.0, 10.0), (Phase::Move, 110.0, 60.0), (Phase::Up, 110.0, 60.0)] {
//! 	canvas.input(&Input::Pointer(PointerEvent::mouse(phase, Point::new(x, y), 0.0)))?;
//! }
//!
//! let rect = canvas.state().selection().nodes()[0];
//! let size = canvas.state().index().get(rect).map(|geometry| geometry.size);
//! assert_eq!(size, Some(kurbo::Vec2::new(100.0, 50.0)));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
pub mod geometry;
pub mod handles;
pub mod inspector;
pub mod layout;
pub mod tools;

mod camera;
mod canvas;
mod extension;
mod input;
mod state;

mod hit;
mod index;
mod selection;
mod snap;
mod transform;

mod kits;
mod overlay;
mod rulers;
mod text;

pub use camera::Camera;
pub use camera::Inertia;
pub use camera::MAX_ZOOM;
pub use camera::MIN_ZOOM;
pub use camera::Pinch;
pub use canvas::Canvas;
pub use canvas::CanvasBuilder;
pub use extension::CanvasExtension;
pub use extension::Command;
pub use extension::CommandFn;
pub use extension::Registration;
pub use extension::ToolId;
pub use extension::TypedCanvasExtension;
pub use handles::Handle;
pub use handles::HandleStyle;
pub use hit::MarqueeMode;
pub use hit::hit_test;
pub use hit::marquee;
pub use hit::paint_order;
pub use hit::selection_target;
pub use index::NodeGeometry;
pub use index::SceneIndex;
pub use input::Input;
pub use input::KeyEvent;
pub use input::Modifiers;
pub use input::Phase;
pub use input::PointerEvent;
pub use input::PointerKind;
pub use input::WheelEvent;
pub use kits::DesignKit;
pub use kits::DesignKitHandle;
pub use kits::Frames;
pub use kits::FramesHandle;
pub use kits::Images;
pub use kits::ImagesHandle;
pub use kits::Shapes;
pub use kits::ShapesHandle;
pub use kits::Texts;
pub use kits::TextsHandle;
pub use overlay::Cursor;
pub use overlay::OutlineKind;
pub use overlay::OverlayItem;
pub use overlay::PointKind;
pub use rulers::Tick;
pub use rulers::ticks;
pub use selection::Frame;
pub use selection::Selection;
pub use snap::Axis;
pub use snap::Guide;
pub use snap::Snap;
pub use snap::Snapper;
pub use state::CanvasError;
pub use state::CanvasState;
pub use state::LocalCommit;
pub use text::TextFrameLayout;
pub use text::TextLayouts;
pub use tools::Outcome;
pub use tools::Tool;
pub use transform::Original;
pub use transform::Originals;
