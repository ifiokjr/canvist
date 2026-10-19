//! The camera: where the infinite canvas is on screen.

use kurbo::Affine;
use kurbo::Point;
use kurbo::Rect;
use kurbo::Size;
use kurbo::Vec2;

/// The smallest zoom the camera allows by default: 1%.
pub const MIN_ZOOM: f64 = 0.01;
/// The largest zoom the camera allows by default: 256×.
pub const MAX_ZOOM: f64 = 256.0;

/// Maps world coordinates to screen pixels: `screen = (world - origin) *
/// zoom`. The world is the scene's coordinate space; the screen is the
/// host's viewport in CSS or device-independent pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
	/// The world point at the top-left corner of the viewport.
	pub origin: Point,
	/// Screen pixels per world unit.
	pub zoom: f64,
	/// The smallest allowed zoom.
	pub min_zoom: f64,
	/// The largest allowed zoom.
	pub max_zoom: f64,
}

impl Default for Camera {
	fn default() -> Self {
		Self {
			origin: Point::ORIGIN,
			zoom: 1.0,
			min_zoom: MIN_ZOOM,
			max_zoom: MAX_ZOOM,
		}
	}
}

impl Camera {
	/// The world-to-screen transform.
	#[must_use]
	pub fn world_to_screen(&self) -> Affine {
		Affine::scale(self.zoom) * Affine::translate(-self.origin.to_vec2())
	}

	/// The screen-to-world transform.
	#[must_use]
	pub fn screen_to_world(&self) -> Affine {
		Affine::translate(self.origin.to_vec2()) * Affine::scale(1.0 / self.zoom)
	}

	/// The world point under screen point `screen`.
	#[must_use]
	pub fn to_world(&self, screen: Point) -> Point {
		self.origin + screen.to_vec2() / self.zoom
	}

	/// The screen point showing world point `world`.
	#[must_use]
	pub fn to_screen(&self, world: Point) -> Point {
		((world - self.origin) * self.zoom).to_point()
	}

	/// The world rectangle a viewport of `size` shows.
	#[must_use]
	pub fn visible(&self, size: Size) -> Rect {
		Rect::from_origin_size(self.origin, size / self.zoom)
	}

	/// Pan by a screen-space delta: dragging content right moves the origin
	/// left.
	pub fn pan(&mut self, screen_delta: Vec2) {
		self.origin -= screen_delta / self.zoom;
	}

	/// Multiply the zoom by `factor`, keeping the world point under screen
	/// point `anchor` fixed, as pinch and ctrl-wheel do.
	pub fn zoom_at(&mut self, anchor: Point, factor: f64) {
		let world = self.to_world(anchor);
		self.zoom = (self.zoom * factor).clamp(self.min_zoom, self.max_zoom);
		self.origin = world - anchor.to_vec2() / self.zoom;
	}

	/// Frame world rectangle `target` in a viewport of `size`, leaving
	/// `padding` screen pixels around it.
	pub fn fit(&mut self, target: Rect, size: Size, padding: f64) {
		let room = Size::new(
			(size.width - 2.0 * padding).max(1.0),
			(size.height - 2.0 * padding).max(1.0),
		);
		let zoom = if target.width() > 0.0 && target.height() > 0.0 {
			(room.width / target.width()).min(room.height / target.height())
		} else {
			self.zoom
		};

		self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
		let center = target.center();
		self.origin = center - (size.to_vec2() / 2.0) / self.zoom;
	}
}

/// How quickly momentum fades: the velocity keeps this fraction of itself
/// per second.
const FRICTION_PER_SECOND: f64 = 0.002;
/// Momentum stops below this speed, in screen pixels per second.
const REST_SPEED: f64 = 8.0;
/// Pointer samples older than this do not count toward release velocity.
const SAMPLE_WINDOW: f64 = 0.1;

/// Momentum after a pan gesture ends.
///
/// Feed pointer positions while dragging, call [`Inertia::release`] on
/// pointer up, then call [`Inertia::step`] every frame with the frame's
/// duration and pan the camera by what it returns. Time is passed in, never
/// read, so the motion is deterministic in tests.
#[derive(Clone, Debug, Default)]
pub struct Inertia {
	samples: Vec<(f64, Point)>,
	velocity: Vec2,
}

impl Inertia {
	/// Record the pointer at `position` (screen) at `time` seconds.
	pub fn track(&mut self, time: f64, position: Point) {
		self.velocity = Vec2::ZERO;
		self.samples.push((time, position));
		self.samples
			.retain(|(sample, _)| time - sample <= SAMPLE_WINDOW);
	}

	/// End the drag and start coasting at the recent pointer velocity.
	pub fn release(&mut self) {
		self.velocity = match (self.samples.first(), self.samples.last()) {
			(Some(&(start, from)), Some(&(end, to))) if end > start => (to - from) / (end - start),
			_ => Vec2::ZERO,
		};

		self.samples.clear();
	}

	/// Stop coasting.
	pub fn stop(&mut self) {
		self.velocity = Vec2::ZERO;
		self.samples.clear();
	}

	/// Whether the camera is still coasting.
	#[must_use]
	pub fn is_moving(&self) -> bool {
		self.velocity.hypot() >= REST_SPEED
	}

	/// Advance `elapsed` seconds, returning the screen delta to pan by, or
	/// `None` once at rest. The distance is the exact integral of
	/// exponentially decaying velocity, so it does not depend on the frame
	/// rate.
	pub fn step(&mut self, elapsed: f64) -> Option<Vec2> {
		if !self.is_moving() || elapsed <= 0.0 {
			self.velocity = Vec2::ZERO;

			return None;
		}

		let rate = FRICTION_PER_SECOND.ln();
		let decay = (rate * elapsed).exp();
		let delta = self.velocity * ((decay - 1.0) / rate);
		self.velocity *= decay;
		Some(delta)
	}
}

/// A two-finger pinch: zoom by the change in finger distance, pan by the
/// movement of their midpoint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pinch {
	distance: f64,
	midpoint: Point,
}

impl Pinch {
	/// Start a pinch with fingers at `a` and `b` (screen).
	#[must_use]
	pub fn start(a: Point, b: Point) -> Self {
		Self {
			distance: a.distance(b).max(1.0),
			midpoint: a.midpoint(b),
		}
	}

	/// The fingers moved to `a` and `b`: apply the change to `camera`.
	pub fn update(&mut self, camera: &mut Camera, a: Point, b: Point) {
		let distance = a.distance(b).max(1.0);
		let midpoint = a.midpoint(b);
		camera.pan(midpoint - self.midpoint);
		camera.zoom_at(midpoint, distance / self.distance);
		self.distance = distance;
		self.midpoint = midpoint;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// How close two points must be to count as the same.
	const EPSILON: f64 = 1e-9;
	/// How close two zoom levels must be to count as the same.
	const ZOOM_EPSILON: f64 = 1e-12;

	fn close(a: Point, b: Point) -> bool {
		a.distance(b) < EPSILON
	}

	#[test]
	fn world_and_screen_round_trip() {
		/// A camera position off the origin on both axes.
		const ORIGIN: Point = Point::new(100.0, -50.0);
		/// A world point away from the camera's origin.
		const WORLD: Point = Point::new(130.0, 10.0);
		/// A screen point near the top-left corner.
		const SCREEN: Point = Point::new(3.0, 4.0);

		let camera = Camera {
			origin: ORIGIN,
			zoom: 2.5,
			..Camera::default()
		};

		assert!(close(camera.to_world(camera.to_screen(WORLD)), WORLD));
		assert!(close(
			camera.world_to_screen() * WORLD,
			camera.to_screen(WORLD)
		));
		assert!(close(
			camera.screen_to_world() * SCREEN,
			camera.to_world(SCREEN)
		));
	}

	#[test]
	fn zooming_keeps_the_anchor_still() {
		/// The centre of a 640 × 480 viewport.
		const ANCHOR: Point = Point::new(320.0, 240.0);

		let mut camera = Camera::default();
		let before = camera.to_world(ANCHOR);
		camera.zoom_at(ANCHOR, 3.0);

		assert!(close(camera.to_world(ANCHOR), before));
		assert!((camera.zoom - 3.0).abs() < ZOOM_EPSILON);

		camera.zoom_at(ANCHOR, 1e9);
		assert!((camera.zoom - MAX_ZOOM).abs() < ZOOM_EPSILON);
	}

	#[test]
	fn fit_centres_the_target() {
		/// A 200 × 100 rectangle far from the origin.
		const TARGET: Rect = Rect::new(1000.0, 1000.0, 1200.0, 1100.0);
		/// The viewport the target is fitted into.
		const VIEWPORT: Size = Size::new(800.0, 600.0);
		/// Screen pixels left around the target.
		const PADDING: f64 = 20.0;
		/// The zoom that fits the target's width into the padded viewport:
		/// (800 - 2 × 20) / 200.
		const FITTED_ZOOM: f64 = 3.8;

		let mut camera = Camera::default();
		camera.fit(TARGET, VIEWPORT, PADDING);

		let viewport_center = (VIEWPORT.to_vec2() / 2.0).to_point();
		assert!(close(camera.to_screen(TARGET.center()), viewport_center));
		assert!((camera.zoom - FITTED_ZOOM).abs() < ZOOM_EPSILON);
	}

	#[test]
	fn momentum_decays_independently_of_frame_rate() {
		/// Seconds between the two tracked pointer samples.
		const SAMPLE_GAP: f64 = 0.05;
		/// Screen pixels the pointer moves between the samples.
		const FLICK: f64 = 50.0;
		/// Seconds to coast for.
		const COAST_TIME: f64 = 0.5;

		let coast = |frame: f64| {
			let mut inertia = Inertia::default();
			inertia.track(0.0, Point::ORIGIN);
			inertia.track(SAMPLE_GAP, Point::new(FLICK, 0.0));
			inertia.release();

			let mut travelled = Vec2::ZERO;
			let mut time = 0.0;

			while time < COAST_TIME {
				travelled += inertia.step(frame).unwrap_or_default();
				time += frame;
			}

			travelled.x
		};

		assert!((coast(1.0 / 60.0) - coast(1.0 / 120.0)).abs() < 1.0);
		assert!(coast(1.0 / 60.0) > 100.0);
	}

	#[test]
	fn a_pinch_zooms_about_its_midpoint() {
		/// Where both fingers rest, and their midpoint.
		const MIDPOINT: Point = Point::new(150.0, 100.0);
		/// Half the starting distance between the fingers.
		const SPREAD: Vec2 = Vec2::new(50.0, 0.0);

		let mut camera = Camera::default();
		let mut pinch = Pinch::start(MIDPOINT - SPREAD, MIDPOINT + SPREAD);
		let under = camera.to_world(MIDPOINT);
		pinch.update(
			&mut camera,
			MIDPOINT - SPREAD * 2.0,
			MIDPOINT + SPREAD * 2.0,
		);

		assert!((camera.zoom - 2.0).abs() < ZOOM_EPSILON);
		assert!(close(camera.to_world(MIDPOINT), under));
	}
}
