//! A deterministic measurer for tests and benchmarks without a platform.

use std::sync::Mutex;
use std::sync::PoisonError;

use canvist_layout::FixedMeasurer;
use canvist_layout::FontSpec;

use crate::MeasureRequest;
use crate::MeasureResponse;
use crate::PlatformMeasurer;
use crate::VerticalMetrics;

/// The size of a font the measurer was never sent, which a correct caller
/// never asks about.
const UNKNOWN_FONT_SIZE: f32 = 16.0;
/// Ascent and descent as shares of the font size.
const ASCENT_EM: f32 = 0.8;
const DESCENT_EM: f32 = 0.2;

/// Measures like [`FixedMeasurer`]: every character `0.6em` wide (`1em`
/// for wide ones), ascent `0.8em`, descent `0.2em`. It counts requests, so
/// tests can check how often the editor measures.
#[derive(Debug, Default)]
pub struct FixedPlatformMeasurer {
	state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
	fonts: Vec<Option<FontSpec>>,
	requests: u64,
}

impl FixedPlatformMeasurer {
	/// How many requests it answered.
	#[must_use]
	pub fn requests(&self) -> u64 {
		self.state
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.requests
	}
}

impl PlatformMeasurer for FixedPlatformMeasurer {
	fn measure(&self, request: MeasureRequest) -> MeasureResponse {
		let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
		state.requests += 1;

		for face in request.fonts {
			let index = face.id as usize;

			if state.fonts.len() <= index {
				state.fonts.resize(index + 1, None);
			}

			state.fonts[index] = Some(FontSpec {
				family: face.families.join(", ").into(),
				size: face.size,
				weight: face.weight,
				italic: face.italic,
			});
		}

		let font = |id: u32| state.fonts.get(id as usize).and_then(Option::as_ref);
		let metrics = request
			.metrics
			.iter()
			.map(|&id| {
				let size = font(id).map_or(UNKNOWN_FONT_SIZE, |font| font.size);
				VerticalMetrics {
					ascent: size * ASCENT_EM,
					descent: size * DESCENT_EM,
				}
			})
			.collect();
		let advances = request
			.segments
			.iter()
			.flat_map(|segment| {
				let font = font(segment.font).cloned();
				segment.text.chars().map(move |c| {
					font.as_ref()
						.map_or(0.0, |font| FixedMeasurer::advance(c, font))
				})
			})
			.collect();

		MeasureResponse { metrics, advances }
	}
}
