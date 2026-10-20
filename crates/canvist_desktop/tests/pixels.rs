//! Screenshots: the desktop editor lays out exactly as canvist's
//! server-side renderer does, and paints the same pixels a CPU rendering of
//! the SSR layout does.
//!
//! Golden images live in `tests/snapshots/`. Rewrite them with
//! `CANVIST_UPDATE_SNAPSHOTS=1 cargo test -p canvist_desktop --test pixels`.

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use canvist_desktop::Viewport;
use canvist_headless::Renderer;
use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::LayoutOptions;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Theme;
use canvist_paint::CpuPainter;
use canvist_paint::DisplayStyle;
use canvist_paint::Draw;
use canvist_paint::peniko::Brush;
use canvist_paint::peniko::Fill;
use kurbo::Affine;
use kurbo::Shape as _;
use support::Driver;

/// The document every screenshot shows: headings, marks, a list, a quote,
/// code, and text that wraps.
const DOCUMENT: &str = "# Field notes

Canvas editors paint **every** glyph themselves, so the *same* layout reaches the screen, a PNG, \
                        and server-rendered HTML.

- One core
- Every platform

> Quiet tools make room for loud ideas.

```
let fast = true;
```";

/// The document column's width, and the editor's size around it.
const COLUMN: f32 = 420.0;
const HEIGHT: f32 = 520.0;
/// The task list screenshot's column width and height.
const TASKS: (f32, f32) = (300.0, 120.0);
/// Channel difference two pixels may have and still count as equal: the
/// rasteriser's float rounding differs a little between CPUs.
const CHANNEL_SLACK: u8 = 8;
/// Share of pixels that may differ by more than that.
const DIFFERENT_SHARE: f64 = 0.002;

fn driver(scale: f32) -> Driver {
	let padding = support::options().padding;
	let mut driver = Driver::sized(DOCUMENT, COLUMN + padding * 2.0, HEIGHT);
	let _ = driver.send(&canvist_desktop::InputEvent::Focus(false));
	driver.editor.resize(Viewport {
		width: COLUMN + padding * 2.0,
		height: HEIGHT,
		scale,
	});
	driver.frame();
	driver
}

/// The SSR renderer's layout of the document: the same measurer, font, and
/// theme as `canvist_headless::Renderer`.
fn ssr_layout(driver: &Driver) -> DisplayList {
	let document = driver.editor.doc().document();
	let mut measurer = ParleyMeasurer::new();
	measurer.register_font(canvist_paint::BUNDLED_FONT.to_vec());
	let mut layout = DocumentLayout::new(
		document,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(COLUMN),
	);
	layout.layout_all(document, &mut measurer).expect("layout");
	layout.display_list(&DisplayOptions::viewport(0.0, layout.height()))
}

#[test]
fn the_reference_layout_is_the_ssr_renderers() {
	let driver = driver(1.0);
	let svg = Renderer::new()
		.svg(driver.editor.doc(), COLUMN)
		.expect("svg");

	assert_eq!(canvist_layout::to_svg(&ssr_layout(&driver)), svg);
}

#[test]
fn the_editor_lays_out_like_the_ssr_renderer() {
	let driver = driver(1.0);

	assert_eq!(
		driver.editor.display_list().items,
		ssr_layout(&driver).items
	);
}

#[test]
fn the_editor_paints_what_the_ssr_layout_paints() {
	for scale in [1.0, 2.0] {
		let driver = driver(scale);
		let (width, height, desktop) = driver.editor.render_rgba8().expect("pixels");
		let reference = paint_reference(&driver, width, height);

		assert_eq!(
			different_pixels(&desktop, &reference),
			0,
			"at {scale}x the desktop editor and the SSR layout paint differently"
		);
	}
}

#[test]
fn the_editor_matches_its_golden_screenshot() {
	let driver = driver(2.0);
	let png = driver.editor.render_png().expect("png");
	let (_, _, pixels) = driver.editor.render_rgba8().expect("pixels");

	assert_golden("field-notes@2x.png", &png, &pixels);
}

#[test]
fn task_items_draw_their_checkboxes() {
	let padding = support::options().padding;
	let (column, height) = TASKS;
	let mut driver = Driver::sized(
		"- [ ] write tests\n- [x] ship it",
		column + padding * 2.0,
		height,
	);
	let _ = driver.send(&canvist_desktop::InputEvent::Focus(false));
	driver.frame();
	let png = driver.editor.render_png().expect("png");
	let (_, _, pixels) = driver.editor.render_rgba8().expect("pixels");

	assert_golden("tasks.png", &png, &pixels);
}

/// The SSR layout painted on the CPU at the editor's padding and scale.
fn paint_reference(driver: &Driver, width: u32, height: u32) -> Vec<u8> {
	let options = driver.editor.options();
	let scale = f64::from(driver.editor.viewport().scale);
	let mut painter = CpuPainter::new(width, height).expect("painter");
	let background = Brush::Solid(canvist_paint::convert::layout_color(
		options.theme.chrome().background,
	));
	let everything = kurbo::Rect::new(0.0, 0.0, f64::from(width), f64::from(height)).to_path(0.1);
	painter_fill(&mut painter, &everything, &background);

	let padding = f64::from(options.padding);
	let to_screen = Affine::scale(scale) * Affine::translate((padding, padding));
	canvist_paint::paint_display_list(
		&mut painter,
		&ssr_layout(driver),
		&DisplayStyle::new(to_screen),
	);
	painter.render_rgba8()
}

fn painter_fill(painter: &mut CpuPainter, path: &kurbo::BezPath, brush: &Brush) {
	use canvist_paint::Painter as _;

	painter.fill(
		&Draw {
			path,
			transform: Affine::IDENTITY,
			brush,
			brush_transform: None,
		},
		Fill::NonZero,
	);
}

fn different_pixels(a: &[u8], b: &[u8]) -> usize {
	assert_eq!(a.len(), b.len(), "the images are the same size");
	a.chunks_exact(4)
		.zip(b.chunks_exact(4))
		.filter(|(a, b)| {
			a.iter()
				.zip(b.iter())
				.any(|(a, b)| a.abs_diff(*b) > CHANNEL_SLACK)
		})
		.count()
}

/// Compare `pixels` with the golden PNG `name`, or write it when updating.
fn assert_golden(name: &str, png: &[u8], pixels: &[u8]) {
	let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("tests/snapshots")
		.join(name);

	if std::env::var_os("CANVIST_UPDATE_SNAPSHOTS").is_some() || !path.exists() {
		std::fs::create_dir_all(path.parent().expect("a parent")).expect("snapshot folder");
		std::fs::write(&path, png).expect("write the golden image");

		return;
	}

	let golden = decode_png(&std::fs::read(&path).expect("the golden image"));
	let different = different_pixels(pixels, &golden);
	let allowed = (pixels.len() / 4) as f64 * DIFFERENT_SHARE;

	assert!(
		different as f64 <= allowed,
		"{different} pixels differ from {name} (allowed {allowed:.0}); rewrite it with \
		 CANVIST_UPDATE_SNAPSHOTS=1 if the change is intended"
	);
}

/// Straight-alpha RGBA8 pixels of a PNG written by `vello_cpu`, which
/// leaves out the alpha channel of opaque images.
fn decode_png(png: &[u8]) -> Vec<u8> {
	let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
	decoder.set_transformations(png::Transformations::ALPHA);
	let mut reader = decoder.read_info().expect("a PNG");
	let mut pixels = vec![0; reader.output_buffer_size().expect("a size")];
	let info = reader.next_frame(&mut pixels).expect("a frame");
	pixels.truncate(info.buffer_size());
	pixels
}
