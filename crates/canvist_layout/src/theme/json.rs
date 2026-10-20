//! StarterKit theme options from JSON, behind the `json` feature: the shape
//! `@canvist/web`'s `theme` option and `@canvist/headless/ssr` take, so the
//! browser and the server read one theme the same way.

use std::sync::Arc;

use serde_json::Value as Json;

use super::ColorScheme;
use super::StarterKitOptions;

impl StarterKitOptions {
	/// Read options from a JSON object, applying each key that is present on
	/// top of the defaults. Values of the wrong type are errors rather than
	/// ignored.
	///
	/// ```json
	/// { "scheme": "dark", "bodyFamily": "Inter, sans-serif", "codeFamily": "monospace", "size": 16, "lineHeight": 1.5, "letterSpacing": 0 }
	/// ```
	///
	/// # Errors
	///
	/// A message naming the key whose value has the wrong type, or a size or
	/// line height that is not a positive number.
	pub fn from_json(theme: &Json) -> Result<Self, String> {
		let mut options = Self::default();

		if let Some(scheme) = scheme(theme)? {
			options.scheme = scheme;
		}

		if let Some(family) = string(theme, "bodyFamily")? {
			options.body_family = Arc::from(family);
		}

		if let Some(family) = string(theme, "codeFamily")? {
			options.code_family = Arc::from(family);
		}

		if let Some(size) = number(theme, "size")? {
			options.size = positive("theme.size", size)?;
		}

		if let Some(line_height) = number(theme, "lineHeight")? {
			options.line_height = positive("theme.lineHeight", line_height)?;
		}

		if let Some(spacing) = number(theme, "letterSpacing")? {
			options.letter_spacing = spacing;
		}

		Ok(options)
	}
}

fn scheme(theme: &Json) -> Result<Option<ColorScheme>, String> {
	match string(theme, "scheme")? {
		None => Ok(None),
		Some("light") => Ok(Some(ColorScheme::Light)),
		Some("dark") => Ok(Some(ColorScheme::Dark)),
		Some(other) => {
			Err(format!(
				"theme.scheme must be \"light\" or \"dark\", not {other:?}"
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

fn positive(name: &str, value: f32) -> Result<f32, String> {
	if value.is_finite() && value > 0.0 {
		Ok(value)
	} else {
		Err(format!("{name} must be a positive number"))
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
	fn empty_object_is_the_default() {
		let options = StarterKitOptions::from_json(&json!({})).expect("valid");

		assert_eq!(options.size, StarterKitOptions::default().size);
		assert_eq!(options.scheme, ColorScheme::Light);
	}

	#[test]
	fn reads_every_key() {
		let options = StarterKitOptions::from_json(&json!({
			"scheme": "dark",
			"bodyFamily": "Lora, serif",
			"codeFamily": "Fira Code, monospace",
			"size": BODY_SIZE,
			"lineHeight": 1.25,
			"letterSpacing": 0.5,
		}))
		.expect("valid");

		assert_eq!(options.scheme, ColorScheme::Dark);
		assert_eq!(&*options.body_family, "Lora, serif");
		assert_eq!(&*options.code_family, "Fira Code, monospace");
		assert_eq!(options.size, BODY_SIZE);
		assert_eq!(options.line_height, 1.25);
		assert_eq!(options.letter_spacing, 0.5);
	}

	#[test]
	fn rejects_wrong_types() {
		for theme in [
			json!({ "scheme": "sepia" }),
			json!({ "size": "16px" }),
			json!({ "size": -1 }),
			json!({ "lineHeight": 0 }),
			json!({ "bodyFamily": 1 }),
		] {
			assert!(StarterKitOptions::from_json(&theme).is_err(), "{theme}");
		}
	}
}
