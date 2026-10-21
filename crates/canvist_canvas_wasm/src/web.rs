//! The `CanvasApp` class JavaScript drives: DOM events in, frames out, and
//! collaboration updates both ways.

use std::time::Duration;

use canvist_canvas::Cursor;
use canvist_canvas::Input;
use canvist_canvas::KeyEvent;
use canvist_canvas::Modifiers;
use canvist_canvas::Phase;
use canvist_canvas::PointerEvent;
use canvist_canvas::PointerKind;
use canvist_canvas::WheelEvent;
use canvist_paint::Quality;
use kurbo::Point;
use kurbo::Size;
use kurbo::Vec2;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::App;
use crate::render::Backend;
use crate::render::Pixels;

/// The Shift bit in a modifier mask.
const SHIFT: u8 = 1;
/// The Alt bit.
const ALT: u8 = 2;
/// The primary bit: Cmd on Apple platforms, Ctrl elsewhere.
const PRIMARY: u8 = 4;

/// The canvist canvas on a `<canvas>` element.
#[wasm_bindgen]
pub struct CanvasApp {
	app: App,
	backend: Backend,
	canvas: HtmlCanvasElement,
	scale: f64,
	/// Whether a pointer is pressed, so frames drawn mid-gesture skip
	/// shadows.
	pressed: bool,
	/// Whether camera momentum is still moving.
	coasting: bool,
}

#[wasm_bindgen]
impl CanvasApp {
	/// Start the canvas on `canvas` as peer `client` (any id but 1), from
	/// `scene` (scene JSON text) or, without one, the demo scene, on WebGPU
	/// when `prefer_gpu` is set and the browser supports it.
	pub async fn create(
		canvas: HtmlCanvasElement,
		client: u32,
		prefer_gpu: bool,
		scene: Option<String>,
	) -> Result<CanvasApp, JsError> {
		let app = match scene {
			Some(scene) => {
				let scene = serde_json::from_str(&scene).map_err(js_error)?;
				App::with_scene(client, &scene)
			}
			None => App::new(client),
		}
		.map_err(js_error)?;
		let backend = Backend::new(&canvas, prefer_gpu).await.map_err(js_error)?;

		Ok(Self {
			app,
			backend,
			canvas,
			scale: 1.0,
			pressed: false,
			coasting: false,
		})
	}

	/// `"webgpu"` or `"cpu"`.
	#[must_use]
	pub fn backend(&self) -> String {
		self.backend.backend().to_owned()
	}

	/// Size the canvas to `width` by `height` CSS pixels at `scale` device
	/// pixels per CSS pixel.
	pub fn resize(&mut self, width: f64, height: f64, scale: f64) {
		self.scale = scale.max(0.25);
		self.app.resize(Size::new(width, height));
		let pixels = device_pixels(width, height, self.scale);
		self.canvas.set_width(pixels.width);
		self.canvas.set_height(pixels.height);
	}

	/// A pointer event: `phase` is `down`, `move`, `up`, or `cancel`, and
	/// `kind` is `mouse`, `pen`, or `touch`. Positions are CSS pixels from
	/// the canvas's top left; `time` is milliseconds. Returns whether to
	/// repaint.
	#[allow(clippy::too_many_arguments)]
	pub fn pointer(
		&mut self,
		phase: &str,
		id: u32,
		kind: &str,
		x: f64,
		y: f64,
		buttons: u8,
		modifiers: u8,
		time: f64,
	) -> Result<bool, JsError> {
		let phase = match phase {
			"down" => Phase::Down,
			"up" => Phase::Up,
			"cancel" => Phase::Cancel,
			_ => Phase::Move,
		};

		self.pressed = buttons != 0 && !matches!(phase, Phase::Up | Phase::Cancel);
		let event = PointerEvent {
			id: u64::from(id),
			kind: pointer_kind(kind),
			phase,
			position: Point::new(x, y),
			buttons,
			modifiers: modifier_keys(modifiers),
			time: time / 1_000.0,
		};
		self.input(&Input::Pointer(event), time)
	}

	/// A wheel event in CSS pixels. `pinch` marks a trackpad pinch, which
	/// browsers report as a wheel event with Ctrl held.
	#[allow(clippy::too_many_arguments)]
	pub fn wheel(
		&mut self,
		dx: f64,
		dy: f64,
		x: f64,
		y: f64,
		modifiers: u8,
		pinch: bool,
		time: f64,
	) -> Result<bool, JsError> {
		let event = WheelEvent {
			delta: Vec2::new(dx, dy),
			position: Point::new(x, y),
			modifiers: modifier_keys(modifiers),
			pinch,
		};
		self.input(&Input::Wheel(event), time)
	}

	/// A key event; `key` is the DOM `KeyboardEvent.key`. Returns whether
	/// the canvas used it, so the host can prevent the browser's default.
	pub fn key(
		&mut self,
		key: &str,
		down: bool,
		modifiers: u8,
		time: f64,
	) -> Result<bool, JsError> {
		let event = KeyEvent {
			key: key.to_owned(),
			modifiers: modifier_keys(modifiers),
			down,
		};
		self.input(&Input::Key(event), time)
	}

	/// Text from an IME or a paste, typed into the open text frame.
	pub fn type_text(&mut self, text: &str, time: f64) -> Result<bool, JsError> {
		self.app.type_text(text, millis(time)).map_err(js_error)
	}

	/// Switch to the tool named `name`. Returns whether one exists.
	pub fn set_tool(&mut self, name: &str) -> Result<bool, JsError> {
		self.app.set_tool(name).map_err(js_error)
	}

	/// The registered tools' names.
	#[must_use]
	pub fn tools(&self) -> Vec<String> {
		self.app
			.canvas()
			.tools()
			.into_iter()
			.map(str::to_owned)
			.collect()
	}

	/// The active tool's name.
	#[must_use]
	pub fn tool(&self) -> String {
		let canvas = self.app.canvas();
		canvas
			.tools()
			.get(canvas.tool().index())
			.map_or_else(String::new, |name| (*name).to_owned())
	}

	/// The CSS cursor the canvas wants.
	#[must_use]
	pub fn cursor(&self) -> String {
		if self.app.session().is_some() {
			return "text".into();
		}

		css_cursor(self.app.canvas().cursor()).to_owned()
	}

	/// Advance camera momentum by `elapsed` seconds. Returns whether to
	/// paint another frame.
	pub fn frame(&mut self, elapsed: f64) -> bool {
		self.coasting = self.app.frame(elapsed);
		self.coasting
	}

	/// Paint and present a frame.
	pub fn render(&mut self) -> Result<(), JsError> {
		let quality = if self.pressed || self.coasting {
			Quality::Interactive
		} else {
			Quality::Full
		};

		let pixels = Pixels {
			width: self.canvas.width(),
			height: self.canvas.height(),
		};
		self.backend
			.render(&self.app, pixels, self.scale, quality)
			.map_err(js_error)
	}

	/// Integrate a collaborator's update. Returns whether anything changed.
	pub fn apply_update(&mut self, update: &[u8]) -> Result<bool, JsError> {
		self.app.apply_update(update).map_err(js_error)
	}

	/// The updates made since the last call, to broadcast.
	pub fn take_updates(&mut self) -> Vec<js_sys::Uint8Array> {
		self.app
			.take_updates()
			.iter()
			.map(|update| js_sys::Uint8Array::from(update.as_slice()))
			.collect()
	}

	/// The shared document's state vector, to ask a peer for what this peer
	/// lacks.
	#[must_use]
	pub fn state_vector(&self) -> Vec<u8> {
		self.app.state_vector()
	}

	/// Everything the peer with state vector `state_vector` lacks.
	pub fn encode_diff(&self, state_vector: &[u8]) -> Result<Vec<u8>, JsError> {
		self.app.encode_diff(state_vector).map_err(js_error)
	}

	/// Add `count` rectangles below the demo: a heavier scene for measuring
	/// frame times.
	pub fn add_stress(&mut self, count: usize) -> Result<(), JsError> {
		self.app.add_stress(count).map_err(js_error)
	}

	/// How many nodes the scene shows.
	#[must_use]
	pub fn node_count(&self) -> usize {
		self.app.canvas().state().scene().len()
	}

	/// How many nodes are selected.
	#[must_use]
	pub fn selected_count(&self) -> usize {
		self.app.canvas().state().selection().nodes().len()
	}

	/// Whether a text frame is open in the text editor.
	#[must_use]
	pub fn editing(&self) -> bool {
		self.app.session().is_some()
	}

	/// The camera zoom.
	#[must_use]
	pub fn zoom(&self) -> f64 {
		self.app.canvas().state().camera().zoom
	}

	/// The whole shared scene as one v1 Yjs update.
	#[must_use]
	pub fn encode_state(&self) -> Vec<u8> {
		self.app.encode_state()
	}

	/// The scene as canonical JSON, to store or to compare peers.
	#[must_use]
	pub fn scene_json(&self) -> String {
		self.app.canvas().state().scene().to_json().to_string()
	}
}

impl CanvasApp {
	fn input(&mut self, input: &Input, time: f64) -> Result<bool, JsError> {
		self.app.input(input, millis(time)).map_err(js_error)
	}
}

/// Milliseconds as a duration, clamped to zero.
fn millis(time: f64) -> Duration {
	Duration::from_secs_f64((time / 1_000.0).max(0.0))
}

fn modifier_keys(mask: u8) -> Modifiers {
	Modifiers {
		shift: mask & SHIFT != 0,
		alt: mask & ALT != 0,
		primary: mask & PRIMARY != 0,
	}
}

fn pointer_kind(kind: &str) -> PointerKind {
	match kind {
		"pen" => {
			PointerKind::Pen {
				pressure: 0.5,
				tilt: Vec2::ZERO,
			}
		}
		"touch" => PointerKind::Touch,
		_ => PointerKind::Mouse,
	}
}

fn device_pixels(width: f64, height: f64, scale: f64) -> Pixels {
	// Canvas sizes are small positive numbers, so they fit a `u32`.
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	let pixels = |css: f64| (css * scale).round().max(1.0) as u32;
	Pixels {
		width: pixels(width),
		height: pixels(height),
	}
}

fn css_cursor(cursor: Cursor) -> &'static str {
	match cursor {
		Cursor::Default => "default",
		Cursor::Grab => "grab",
		Cursor::Grabbing => "grabbing",
		Cursor::Crosshair => "crosshair",
		Cursor::Text => "text",
		Cursor::Move => "move",
		Cursor::Resize(angle) => resize_cursor(angle),
		Cursor::Rotate(_) => "alias",
	}
}

/// The CSS resize cursor nearest `angle` radians.
fn resize_cursor(angle: f64) -> &'static str {
	const CURSORS: [&str; 4] = ["ew-resize", "nwse-resize", "ns-resize", "nesw-resize"];
	let eighth = (angle.rem_euclid(std::f64::consts::PI) / std::f64::consts::FRAC_PI_4).round();
	// `eighth` is between 0 and 4.
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	let index = eighth as usize % CURSORS.len();
	CURSORS[index]
}

fn js_error(error: impl std::fmt::Display) -> JsError {
	JsError::new(&error.to_string())
}
