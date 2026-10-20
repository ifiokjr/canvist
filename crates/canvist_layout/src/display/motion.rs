//! Kinetic text: where each glyph of moving text is, and its colour, at a
//! moment on the animation clock.

use std::f32::consts::TAU;

use crate::Color;
use crate::theme::Motion;

/// How far wave and shake move a glyph, as a fraction of the font size.
const WAVE_HEIGHT: f32 = 0.16;
const SHAKE_DISTANCE: f32 = 0.05;
/// Cycles per second, and how far apart neighbouring glyphs are in a cycle.
const WAVE_SPEED: f32 = 0.9;
const WAVE_STEP: f32 = 0.45;
const PULSE_SPEED: f32 = 0.8;
const PULSE_STEP: f32 = 0.35;
/// Shake picks a new offset this many times a second.
const SHAKE_RATE: f32 = 14.0;
/// The faintest a pulsing glyph gets, as a fraction of its alpha.
const PULSE_FLOOR: f32 = 0.35;
/// Degrees of hue per second, and between neighbouring glyphs.
const RAINBOW_SPEED: f32 = 90.0;
const RAINBOW_STEP: f32 = 24.0;

/// Glyph `index` of moving text at `time` seconds: its offset in pixels and
/// its colour.
pub(crate) fn glyph(
	motion: Motion,
	time: f32,
	index: usize,
	size: f32,
	color: Color,
) -> ((f32, f32), Color) {
	let index = index as f32;

	match motion {
		Motion::None => ((0.0, 0.0), color),
		Motion::Wave => {
			let phase = TAU * time * WAVE_SPEED - index * WAVE_STEP;
			((0.0, -phase.sin() * WAVE_HEIGHT * size), color)
		}
		Motion::Shake => {
			let frame = (time * SHAKE_RATE).floor();
			let distance = SHAKE_DISTANCE * size;
			let x = jitter(frame, index, 1.0) * distance;
			let y = jitter(frame, index, 2.0) * distance;
			((x, y), color)
		}
		Motion::Pulse => {
			let phase = TAU * time * PULSE_SPEED - index * PULSE_STEP;
			let strength = PULSE_FLOOR + (1.0 - PULSE_FLOOR) * (0.5 + 0.5 * phase.cos());
			((0.0, 0.0), faded(color, strength))
		}
		Motion::Rainbow => {
			let hue = (time * RAINBOW_SPEED + index * RAINBOW_STEP).rem_euclid(360.0);
			((0.0, 0.0), hued(hue, color))
		}
	}
}

/// A repeatable value from -1 to 1 for a frame, glyph, and axis.
fn jitter(frame: f32, index: f32, axis: f32) -> f32 {
	let seed = (frame * 12.989_8 + index * 78.233 + axis * 37.719).sin() * 43_758.547;
	seed.fract() * 2.0 - 1.0
}

fn faded(color: Color, strength: f32) -> Color {
	Color {
		a: (f32::from(color.a) * strength).round() as u8,
		..color
	}
}

/// A saturated colour of `hue` degrees, keeping `color`'s alpha.
fn hued(hue: f32, color: Color) -> Color {
	let sector = hue / 60.0;
	let rising = (1.0 - (sector.rem_euclid(2.0) - 1.0).abs()) * 0.85;
	let (red, green, blue) = match sector as u32 {
		0 => (0.85, rising, 0.0),
		1 => (rising, 0.85, 0.0),
		2 => (0.0, 0.85, rising),
		3 => (0.0, rising, 0.85),
		4 => (rising, 0.0, 0.85),
		_ => (0.85, 0.0, rising),
	};
	let channel = |value: f32| (value * 255.0).round() as u8;

	Color::rgba(channel(red), channel(green), channel(blue), color.a)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn still_text_never_moves() {
		let ink = Color::rgb(0, 0, 0);

		assert_eq!(glyph(Motion::None, 3.0, 4, 16.0, ink), ((0.0, 0.0), ink));
	}

	#[test]
	fn waves_rise_within_their_height_and_lag_glyph_to_glyph() {
		let ink = Color::rgb(0, 0, 0);
		let at = |time, index| glyph(Motion::Wave, time, index, 20.0, ink).0.1;

		for step in 0..50 {
			assert!(at(step as f32 * 0.07, 3).abs() <= WAVE_HEIGHT * 20.0 + f32::EPSILON);
		}

		assert!(
			(at(0.3, 0) - at(0.3, 1)).abs() > 0.1,
			"neighbours are out of step"
		);
	}

	#[test]
	fn shake_is_repeatable_and_rainbow_keeps_alpha() {
		let ink = Color::rgba(10, 20, 30, 128);

		assert_eq!(
			glyph(Motion::Shake, 1.234, 2, 16.0, ink),
			glyph(Motion::Shake, 1.234, 2, 16.0, ink)
		);
		assert_eq!(glyph(Motion::Rainbow, 0.5, 1, 16.0, ink).1.a, 128);
		assert!(glyph(Motion::Pulse, 0.0, 0, 16.0, ink).1.a <= 128);
	}
}
