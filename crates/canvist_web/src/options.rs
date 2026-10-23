//! Editor options, passed from JavaScript as JSON.

use std::sync::Arc;

use canvist_headless::canvist_core::Platform;
use canvist_layout::BreakOverride;
use canvist_layout::StarterKitOptions;
use canvist_layout::Theme;
use serde_json::Value as Json;

/// Which browser's line breaking to match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Breaks {
	/// Blink's tailoring of UAX #14.
	Chromium,
	/// ICU's defaults, which Firefox and WebKit follow.
	Icu,
}

impl Breaks {
	pub(crate) fn override_fn(self) -> Option<BreakOverride> {
		match self {
			Self::Chromium => Some(canvist_layout::chromium_breaks),
			Self::Icu => None,
		}
	}
}

/// Everything `new WebEditor(…)` is configured with.
#[derive(Clone, Debug)]
pub(crate) struct Options {
	/// The client id new blocks are allocated under.
	pub(crate) client: u32,
	/// The theme.
	pub(crate) theme: Arc<Theme>,
	/// The line-breaking tailoring.
	pub(crate) breaks: Breaks,
	/// How far beyond the viewport to lay out, in pixels.
	pub(crate) overscan: f32,
	/// Whether `Mod` in key bindings is Cmd (`"apple"`) or Ctrl.
	pub(crate) platform: Platform,
}

/// Read options from JSON, rejecting values of the wrong type instead of
/// ignoring them.
///
/// ```json
/// {
///   "client": 1,
///   "breaks": "chromium",
///   "overscan": 600,
///   "platform": "apple",
///   "theme": { "scheme": "dark", "bodyFamily": "Inter, sans-serif", "codeFamily": "monospace", "size": 16, "lineHeight": 1.5 }
/// }
/// ```
pub(crate) fn parse(json: &Json) -> Result<Options, String> {
	let theme = json.get("theme").unwrap_or(&Json::Null);
	let starter = StarterKitOptions::from_json(theme)?;
	let breaks = breaks(json)?;

	Ok(Options {
		client: number(json, "client")?.map_or(1, |client| client as u32),
		theme: Arc::new(Theme::starter_kit_with(&starter)),
		breaks,
		overscan: number(json, "overscan")?.unwrap_or(0.0),
		platform: platform(json)?,
	})
}

fn breaks(json: &Json) -> Result<Breaks, String> {
	match string(json, "breaks")? {
		None | Some("chromium") => Ok(Breaks::Chromium),
		Some("icu") => Ok(Breaks::Icu),
		Some(other) => {
			Err(format!(
				"breaks must be \"chromium\" or \"icu\", not {other:?}"
			))
		}
	}
}

fn platform(json: &Json) -> Result<Platform, String> {
	match string(json, "platform")? {
		None | Some("other") => Ok(Platform::Other),
		Some("apple") => Ok(Platform::Apple),
		Some(other) => {
			Err(format!(
				"platform must be \"apple\" or \"other\", not {other:?}"
			))
		}
	}
}

fn string<'a>(json: &'a Json, key: &str) -> Result<Option<&'a str>, String> {
	match json.get(key) {
		None | Some(Json::Null) => Ok(None),
		Some(Json::String(value)) => Ok(Some(value)),
		Some(_) => Err(format!("{key} must be a string")),
	}
}

fn number(json: &Json, key: &str) -> Result<Option<f32>, String> {
	match json.get(key) {
		None | Some(Json::Null) => Ok(None),
		Some(Json::Number(value)) => Ok(value.as_f64().map(|value| value as f32)),
		Some(_) => Err(format!("{key} must be a number")),
	}
}

#[cfg(test)]
#[allow(
	clippy::float_cmp,
	reason = "the sizes compared are exactly representable"
)]
mod tests {
	use serde_json::json;

	use super::*;

	/// A body size other than the starter kit's default.
	const BODY_SIZE: f32 = 18.0;

	#[test]
	fn defaults() {
		let options = parse(&json!({})).expect("valid");

		assert_eq!(options.client, 1);
		assert_eq!(options.breaks, Breaks::Chromium);
		assert_eq!(options.theme.base().font.size, 16.0);
		assert_eq!(options.platform, Platform::Other);
		assert_eq!(
			parse(&json!({ "platform": "apple" }))
				.expect("valid")
				.platform,
			Platform::Apple
		);
	}

	#[test]
	fn theme_options() {
		let options = parse(&json!({
			"breaks": "icu",
			"theme": { "scheme": "dark", "bodyFamily": "Lora, serif", "size": BODY_SIZE }
		}))
		.expect("valid");

		assert_eq!(options.breaks, Breaks::Icu);
		assert_eq!(&*options.theme.base().font.family, "Lora, serif");
		assert_eq!(options.theme.base().font.size, BODY_SIZE);
		assert_ne!(
			options.theme.chrome().background,
			Theme::starter_kit().chrome().background
		);
	}

	#[test]
	fn rejects_wrong_types() {
		assert!(parse(&json!({ "theme": { "scheme": "sepia" } })).is_err());
		assert!(parse(&json!({ "theme": { "size": "16px" } })).is_err());
		assert!(parse(&json!({ "theme": { "size": -1 } })).is_err());
		assert!(parse(&json!({ "breaks": 1 })).is_err());
		assert!(parse(&json!({ "platform": "mac" })).is_err());
	}
}
