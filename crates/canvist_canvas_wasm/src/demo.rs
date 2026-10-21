//! The demo scene: one of each kind of node, built the same way in every
//! tab.
//!
//! The scene is [`DEMO_SCENE`], written in the nested scene JSON format an
//! agent would write. Every tab builds it under the same client id, so
//! every tab's shared document is byte-for-byte the same and merging two of
//! them is a no-op. Tabs then edit under their own client ids. Nobody has
//! to decide who seeds the room, and a tab that opens alone works straight
//! away.

use std::sync::Arc;

use canvist_collab::Encoding;
use canvist_collab::SceneBinding;
use canvist_paint::Assets;
use canvist_scene::Scene;
use canvist_scene::SceneSchema;

use crate::AppError;

/// The client id every starting scene is built under, the demo's or one
/// the host passes. Tabs must use another.
pub const SEED_CLIENT: u32 = 1;
/// The demo scene, as nested scene JSON.
pub const DEMO_SCENE: &str = include_str!("demo.json");
/// The asset id the demo scene's image uses.
pub const SUNSET: &str = "demo:sunset";

/// The sunset image's side in pixels.
const SUNSET_SIDE: u32 = 256;
/// The horizon, in rows from the top.
const HORIZON: u32 = SUNSET_SIDE * 5 / 8;
/// The sun's radius, as a fraction of the image's side.
const SUN_SIZE: f64 = 0.2;
/// Rows from one band of the sun's reflection to the next.
const GLINT_PERIOD: u32 = 12;
/// Lit rows in each band.
const GLINT_ROWS: u32 = 3;
/// How far the reflection reaches from the centre at the horizon, in
/// pixels.
const GLINT_REACH: u32 = 40;
/// Rows per pixel the reflection narrows by towards the viewer.
const GLINT_TAPER: u32 = 3;

const SKY_TOP: [u8; 3] = [40, 33, 120];
const SKY_BOTTOM: [u8; 3] = [255, 140, 90];
const SUN: [u8; 3] = [255, 214, 120];
const GLINT: [u8; 3] = [255, 190, 110];
const SEA: [u8; 3] = [30, 40, 90];

/// The shared state, v1 encoded, of `scene` (scene JSON) built under
/// [`SEED_CLIENT`]: the same bytes in every tab that starts from the same
/// JSON, so their documents merge without duplicating anything.
pub fn seed_state(
	schema: &Arc<SceneSchema>,
	scene: &serde_json::Value,
) -> Result<Vec<u8>, AppError> {
	let scene = Scene::from_json(schema.clone(), SEED_CLIENT, scene)?;
	let mut binding = SceneBinding::with_client_id(schema.clone(), u64::from(SEED_CLIENT))?;
	binding.seed(&scene)?;
	Ok(binding.encode_state(Encoding::V1))
}

/// Decoded images the demo scene uses.
#[must_use]
pub fn assets() -> Assets {
	let mut assets = Assets::new();
	assets.insert_rgba8(SUNSET, SUNSET_SIDE, SUNSET_SIDE, sunset());
	assets
}

/// A square sunset in RGBA8: a sky gradient, a sun on the horizon, and its
/// reflection on a dark sea.
fn sunset() -> Vec<u8> {
	(0..SUNSET_SIDE)
		.flat_map(|y| (0..SUNSET_SIDE).map(move |x| pixel(x, y)))
		.flat_map(|[red, green, blue]| [red, green, blue, u8::MAX])
		.collect()
}

fn pixel(x: u32, y: u32) -> [u8; 3] {
	if y >= HORIZON {
		return sea(x, y - HORIZON);
	}

	let side = f64::from(SUNSET_SIDE);
	let from_sun = (f64::from(x) - side / 2.0).hypot(f64::from(y) - f64::from(HORIZON));

	if from_sun < side * SUN_SIZE {
		return SUN;
	}

	mix(SKY_TOP, SKY_BOTTOM, f64::from(y) / f64::from(HORIZON))
}

/// The sea `depth` rows below the horizon: bands of reflected sun that
/// narrow towards the viewer.
fn sea(x: u32, depth: u32) -> [u8; 3] {
	let reach = GLINT_REACH.saturating_sub(depth / GLINT_TAPER);
	let lit = depth % GLINT_PERIOD < GLINT_ROWS && x.abs_diff(SUNSET_SIDE / 2) < reach;

	if lit { GLINT } else { SEA }
}

/// Interpolate between `a` and `b` by `t` in `[0, 1]`.
fn mix(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
	// The result stays between the endpoints, so it fits a `u8`.
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	let channel =
		|i: usize| (f64::from(a[i]) + (f64::from(b[i]) - f64::from(a[i])) * t).round() as u8;
	[channel(0), channel(1), channel(2)]
}
