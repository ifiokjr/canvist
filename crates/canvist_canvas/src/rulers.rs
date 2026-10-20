//! Ruler ticks for the current camera.

use crate::Camera;
use crate::snap::Axis;

/// Screen pixels a major step needs for ten minor divisions.
const TEN_DIVISIONS_PIXELS: f64 = 100.0;
/// Screen pixels a major step needs for five minor divisions.
const FIVE_DIVISIONS_PIXELS: f64 = 40.0;

/// One tick on a ruler.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
	/// The world coordinate.
	pub world: f64,
	/// The screen coordinate along the ruler.
	pub screen: f64,
	/// Whether this is a labelled major tick.
	pub major: bool,
}

/// Ticks for a ruler along `axis`, `length` screen pixels long, with major
/// ticks at least `spacing` pixels apart at a round world step (1, 2, or 5
/// times a power of ten) and minor ticks between them.
#[must_use]
pub fn ticks(camera: &Camera, axis: Axis, length: f64, spacing: f64) -> Vec<Tick> {
	let major = nice_step(spacing / camera.zoom);
	let minor = major / minor_divisions(major * camera.zoom);

	let origin = match axis {
		Axis::X => camera.origin.x,
		Axis::Y => camera.origin.y,
	};

	let first = (origin / minor).floor() as i64;
	let last = ((origin + length / camera.zoom) / minor).ceil() as i64;
	let per_major = (major / minor).round() as i64;

	(first..=last)
		.map(|step| {
			let world = step as f64 * minor;
			Tick {
				world,
				screen: (world - origin) * camera.zoom,
				major: step.rem_euclid(per_major.max(1)) == 0,
			}
		})
		.collect()
}

/// The smallest of 1, 2, or 5 times a power of ten that is at least `at_least`.
fn nice_step(at_least: f64) -> f64 {
	let power = 10f64.powf(at_least.max(f64::MIN_POSITIVE).log10().floor());

	[1.0, 2.0, 5.0, 10.0]
		.into_iter()
		.map(|factor| factor * power)
		.find(|&step| step >= at_least)
		.unwrap_or(10.0 * power)
}

/// How many minor ticks fit in a major step `pixels` wide.
fn minor_divisions(pixels: f64) -> f64 {
	if pixels >= TEN_DIVISIONS_PIXELS {
		10.0
	} else if pixels >= FIVE_DIVISIONS_PIXELS {
		5.0
	} else {
		2.0
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn steps_are_round() {
		assert!((nice_step(0.3) - 0.5).abs() < 1e-12);
		assert!((nice_step(7.0) - 10.0).abs() < 1e-12);
		assert!((nice_step(120.0) - 200.0).abs() < 1e-12);
	}

	#[test]
	fn ticks_follow_the_camera() {
		/// The ruler's length in screen pixels.
		const LENGTH: f64 = 200.0;
		/// The least screen distance between major ticks.
		const SPACING: f64 = 80.0;
		/// How close screen and world coordinates must be to agree.
		const EPSILON: f64 = 1e-9;

		let camera = Camera::default();
		let ticks = ticks(&camera, Axis::X, LENGTH, SPACING);
		let majors: Vec<f64> = ticks
			.iter()
			.filter(|tick| tick.major)
			.map(|tick| tick.world)
			.collect();

		assert_eq!(majors, [0.0, 100.0, 200.0]);
		// At zoom 1 with the origin at 0, screen and world coordinates agree.
		let aligned = ticks
			.iter()
			.all(|tick| (tick.screen - tick.world).abs() < EPSILON);
		assert!(aligned);
	}
}
