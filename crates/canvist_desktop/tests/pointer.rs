//! Mouse and trackpad selection, scrolling, and high-DPI, driven
//! headlessly.

mod support;

use std::time::Duration;

use canvist_desktop::InputEvent;
use canvist_desktop::PointerButton;
use canvist_desktop::Viewport;
use kurbo::Point;
use support::Driver;
use support::NONE;
use support::SHIFT;

/// Lines in the long documents that scroll.
const LINES: usize = 100;
/// The editor's size in the high-DPI test, and its scale.
const SMALL: (f32, f32) = (300.0, 120.0);
const RETINA: f32 = 2.0;
/// A frame's worth of time while autoscrolling.
const FRAME: Duration = Duration::from_millis(16);

/// The middle of the first line, in the editor's coordinates.
fn first_line_y(driver: &Driver) -> f64 {
	driver.point_of(0, 0).1
}

/// The x of the start of `text` in the first paragraph.
fn x_before(driver: &Driver, text: &str) -> f64 {
	driver.point_of(0, offset_of(driver, text)).0
}

/// The x of the end of `text` in the first paragraph.
fn x_after(driver: &Driver, text: &str) -> f64 {
	driver
		.point_of(0, offset_of(driver, text) + text.len() as u32)
		.0
}

fn offset_of(driver: &Driver, text: &str) -> u32 {
	let block = driver.editor.doc().document().children(None)[0];
	let content = driver.editor.doc().document().inline(block).expect("text");
	content
		.as_str()
		.find(text)
		.expect("the text is in the paragraph") as u32
}

/// A document of [`LINES`] short paragraphs, taller than the editor.
fn long_document() -> String {
	let lines: Vec<String> = (0..LINES).map(|index| format!("Line {index}")).collect();
	lines.join("\n\n")
}

#[test]
fn a_click_places_the_caret() {
	let mut driver = Driver::new("Hello world");
	let y = first_line_y(&driver);

	driver.click(x_before(&driver, "world") + 1.0, y, 1);

	let (anchor, head) = driver.editor.doc().selected_range().expect("a caret");
	let world = offset_of(&driver, "world");
	assert_eq!((anchor.offset, head.offset), (world, world));
}

#[test]
fn a_double_click_selects_a_word_and_a_triple_click_the_block() {
	let mut driver = Driver::new("Hello wide world\n\nNext");
	let y = first_line_y(&driver);
	let x = x_before(&driver, "de");

	driver.click(x, y, 2);
	assert_eq!(driver.selected_text(), "wide");

	driver.click(x, y, 1);
	assert_eq!(driver.selected_text(), "Hello wide world");
}

#[test]
fn slow_clicks_do_not_count_up() {
	let mut driver = Driver::new("Hello wide world");
	let (x, y) = (x_before(&driver, "de"), first_line_y(&driver));

	driver.click(x, y, 1);
	driver.now += Duration::from_secs(1);
	driver.click(x, y, 1);

	assert_eq!(driver.selected_text(), "");
}

#[test]
fn shift_click_extends_from_the_anchor() {
	let mut driver = Driver::new("Hello wide world");
	let y = first_line_y(&driver);
	driver.click(x_before(&driver, "Hello"), y, 1);
	driver.now += Duration::from_secs(1);

	let _ = driver.down(x_after(&driver, "Hello"), y, SHIFT);
	let _ = driver.up();

	assert_eq!(driver.selected_text(), "Hello");
}

#[test]
fn dragging_selects() {
	let mut driver = Driver::new("Hello wide world");
	let y = first_line_y(&driver);

	let _ = driver.down(x_before(&driver, "wide"), y, NONE);
	assert!(driver.editor.is_dragging());
	let _ = driver.move_to(x_after(&driver, "wide"), y);
	let _ = driver.move_to(x_after(&driver, "world"), y);
	let _ = driver.up();

	assert!(!driver.editor.is_dragging());
	assert_eq!(driver.selected_text(), "wide world");

	// Moving after release selects nothing more.
	let _ = driver.move_to(x_before(&driver, "Hello"), y);
	assert_eq!(driver.selected_text(), "wide world");
}

#[test]
fn dragging_backwards_selects_from_the_press() {
	let mut driver = Driver::new("Hello wide world");
	let y = first_line_y(&driver);

	let _ = driver.down(x_after(&driver, "wide"), y, NONE);
	let _ = driver.move_to(x_before(&driver, "Hello"), y);
	let _ = driver.up();

	assert_eq!(driver.selected_text(), "Hello wide");
}

#[test]
fn dragging_past_the_bottom_autoscrolls() {
	let mut driver = Driver::sized(&long_document(), support::HEIGHT, support::SHORT);
	let y = first_line_y(&driver);
	let below = f64::from(support::SHORT) * 1.2;

	let _ = driver.down(x_before(&driver, "Line"), y, NONE);
	let _ = driver.move_to(x_before(&driver, "Line"), below);

	// Half a second of frames.
	for _ in 0..30 {
		driver.now += FRAME;
		assert!(driver.editor.autoscroll(driver.now).expect("autoscroll"));
	}

	assert!(
		driver.editor.scroll_top() > 100.0,
		"{}",
		driver.editor.scroll_top()
	);
	let (_, head) = driver.editor.doc().selected_range().expect("a range");
	assert!(head.path[0] > 5, "the selection followed: {head:?}");

	let _ = driver.up();
	assert!(!driver.editor.autoscroll(driver.now).expect("autoscroll"));
}

#[test]
fn the_wheel_scrolls_within_the_document() {
	let mut driver = Driver::sized(&long_document(), support::HEIGHT, support::SHORT);

	assert!(driver.scroll(-250.0).repaint);
	driver.frame();
	assert!((driver.editor.scroll_top() - 250.0).abs() < 1.0);

	let _ = driver.send(&InputEvent::Scroll(canvist_desktop::ScrollDelta::Lines {
		x: 0.0,
		y: 1_000.0,
	}));
	assert!(
		driver.editor.scroll_top().abs() < f32::EPSILON,
		"clamped at the top"
	);
	assert!(!driver.scroll(10.0).repaint, "nothing above the top");
}

#[test]
fn typing_at_the_bottom_keeps_the_caret_in_view() {
	let markdown: Vec<String> = (0..60).map(|index| format!("Line {index}")).collect();
	let mut driver = Driver::sized(&markdown.join("\n\n"), support::HEIGHT, support::SHORT);

	let _ = driver.press("End", support::CTRL);
	driver.frame();
	let (_, y) = driver.caret();

	assert!(driver.editor.scroll_top() > 1_000.0);
	assert!((0.0..300.0).contains(&y), "the caret is on screen at {y}");
}

#[test]
fn secondary_clicks_focus_without_selecting() {
	let mut driver = Driver::new("Hello");
	let _ = driver.send(&InputEvent::Focus(false));
	let before = driver.editor.doc().selection();

	let response = driver.send(&InputEvent::PointerDown {
		position: Point::new(x_before(&driver, "lo"), first_line_y(&driver)),
		button: PointerButton::Secondary,
		modifiers: NONE,
	});

	assert!(response.repaint);
	assert!(driver.editor.is_focused());
	assert_eq!(driver.editor.doc().selection(), before);
}

#[test]
fn high_dpi_renders_at_device_pixels() {
	let (width, height) = SMALL;
	let mut driver = Driver::sized("Sharp", width, height);
	driver.editor.resize(Viewport {
		width,
		height,
		scale: RETINA,
	});
	driver.frame();

	let (width, height, pixels) = driver.editor.render_rgba8().expect("pixels");

	assert_eq!((width, height), (600, 240));
	assert_eq!(pixels.len(), 600 * 240 * 4);
	assert_eq!(driver.editor.pixel_size(), (600, 240));
}
