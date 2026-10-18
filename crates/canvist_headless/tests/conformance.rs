//! Runs the language-neutral fixtures in `conformance/fixtures` against the
//! Rust API. The TypeScript and Dart packages run the same files; see
//! `conformance/readme.md` for the format.

#![cfg(feature = "starter-kit")]

use std::path::Path;
use std::path::PathBuf;

use canvist_extensions::StarterKitHandles;
use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use canvist_headless::Snapshot;
use serde_json::Value as Json;
use serde_json::json;

/// The optional features this build has.
const FEATURES: &[&str] = if cfg!(feature = "collab") {
	&["collab"]
} else {
	&[]
};

/// Every case in every file runs, and all failures are reported together,
/// so one broken binding behaviour does not hide the others. Cases needing a
/// feature this build lacks are counted as skipped, never as passed.
#[test]
fn every_fixture_passes() {
	let mut failures = Vec::new();
	let mut passed = 0;
	let mut skipped = 0;

	for path in fixture_files() {
		let file = read_json(&path);
		let cases = file["cases"].as_array().expect("a `cases` array");

		for case in cases {
			let name = format!(
				"{}: {}",
				file_name(&path),
				case["name"].as_str().unwrap_or("?")
			);

			if !supported(case) {
				skipped += 1;
				continue;
			}

			match run_case(case) {
				Ok(()) => passed += 1,
				Err(reason) => failures.push(format!("{name}\n    {reason}")),
			}
		}
	}

	eprintln!(
		"conformance: {passed} passed, {skipped} skipped, {} failed",
		failures.len()
	);
	assert!(
		failures.is_empty(),
		"failing cases:\n{}",
		failures.join("\n")
	);
	assert!(passed > 0, "no fixtures ran");
}

fn fixture_files() -> Vec<PathBuf> {
	let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures");
	let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
		.expect("the conformance fixtures directory")
		.map(|entry| entry.expect("a directory entry").path())
		.filter(|path| {
			path.extension()
				.is_some_and(|extension| extension == "json")
		})
		.collect();
	files.sort();
	files
}

fn read_json(path: &Path) -> Json {
	let text = std::fs::read_to_string(path).expect("a readable fixture");
	serde_json::from_str(&text).expect("valid fixture JSON")
}

fn file_name(path: &Path) -> String {
	path.file_stem()
		.map(|stem| stem.to_string_lossy().into_owned())
		.unwrap_or_default()
}

fn supported(case: &Json) -> bool {
	case["requires"]
		.as_array()
		.into_iter()
		.flatten()
		.all(|feature| FEATURES.contains(&feature.as_str().unwrap_or_default()))
}

fn engine() -> Engine<StarterKitHandles> {
	Engine::starter_kit()
}

fn run_case(case: &Json) -> Result<(), String> {
	let input = &case["input"];
	let loaded = load(input);

	if let Some(code) = input["error"].as_str() {
		return expect_error(loaded.map(drop), code, "loading");
	}

	let mut doc = loaded.map_err(|error| format!("loading failed: {error}"))?;

	for (index, step) in case["steps"].as_array().into_iter().flatten().enumerate() {
		run_step(&mut doc, step).map_err(|reason| format!("step {index} ({step}): {reason}"))?;
	}

	check(&doc, &case["expect"]).map_err(|reason| format!("final expectation: {reason}"))
}

fn load(input: &Json) -> Result<Doc, HeadlessError> {
	let engine = engine();

	match input["format"].as_str() {
		None => engine.create(),
		Some("json") => engine.from_json(&input["content"]),
		Some("yjs") => load_yjs(&engine, input),
		Some(name) => engine.parse(name.parse()?, input["content"].as_str().unwrap_or_default()),
	}
}

#[cfg(feature = "collab")]
fn load_yjs(engine: &Engine<StarterKitHandles>, input: &Json) -> Result<Doc, HeadlessError> {
	engine.from_yjs(
		&base64(&input["content"]),
		input["clientId"].as_u64().unwrap_or(1),
	)
}

#[cfg(not(feature = "collab"))]
fn load_yjs(_: &Engine<StarterKitHandles>, _: &Json) -> Result<Doc, HeadlessError> {
	unreachable!("cases loading Yjs require `collab`")
}

fn run_step(doc: &mut Doc, step: &Json) -> Result<(), String> {
	let Some(code) = step["error"].as_str() else {
		return apply(doc, step);
	};

	let before = doc.export(Format::Json);
	expect_error(apply_op(doc, step).map(drop), code, "the step")?;

	if doc.export(Format::Json) != before {
		return Err("the failed step changed the document".to_owned());
	}

	Ok(())
}

fn expect_error(result: Result<(), HeadlessError>, code: &str, what: &str) -> Result<(), String> {
	match result {
		Ok(()) => Err(format!("{what} succeeded, expected error `{code}`")),
		Err(error) if error.code().as_str() == code => Ok(()),
		Err(error) => {
			Err(format!(
				"{what} failed with `{}` ({error}), expected `{code}`",
				error.code()
			))
		}
	}
}

/// Run a step whose outcome must match its expectations.
fn apply(doc: &mut Doc, step: &Json) -> Result<(), String> {
	match step["op"].as_str() {
		Some("expect") => return check(doc, step),
		Some("snapshotRoundTrip") => return round_trip(doc),
		#[cfg(feature = "collab")]
		Some("rejoin") => return rejoin(doc, step),
		_ => {}
	}

	let outcome =
		apply_op(doc, step).map_err(|error| format!("failed with `{}`: {error}", error.code()))?;
	let expected = expected_outcome(step);

	match expected {
		Some(expected) if expected != outcome => {
			Err(format!("returned {outcome}, expected {expected}"))
		}
		_ => Ok(()),
	}
}

/// The outcome a step declares: `applied`, `changed`, `expect`, or the
/// counts of a sync or update step.
fn expected_outcome(step: &Json) -> Option<Json> {
	match step["op"].as_str()? {
		"receiveSync" => Some(json!([step["reply"], step["broadcast"], step["changed"]])),
		"validate" => Some(json!([step["rejectedBlocks"], step["waiting"]])),
		"takeUpdates" => Some(step["count"].clone()),
		"can" | "query" => Some(step["expect"].clone()),
		"applyUpdate" => step.get("changed").cloned(),
		_ => step.get("applied").cloned(),
	}
}

/// Run one operation, returning what it reports.
fn apply_op(doc: &mut Doc, step: &Json) -> Result<Json, HeadlessError> {
	let text = || step["text"].as_str().unwrap_or_default();
	let args = &step["args"];

	let outcome = match step["op"].as_str().unwrap_or_default() {
		"select" => {
			let anchor = position(&step["anchor"]);
			let head = step.get("head").map_or_else(|| anchor.clone(), position);
			doc.select_range(&anchor, &head).map(|()| true)?
		}
		"selectAll" => doc.select_all().map(|()| true)?,
		"insertText" => doc.insert_text(text())?,
		"typeText" => doc.type_text(text())?,
		"deleteSelection" => doc.delete_selection()?,
		"press" => doc.press(step["key"].as_str().unwrap_or_default())?,
		"paste" => {
			let format: Format = step["format"].as_str().unwrap_or_default().parse()?;
			doc.paste(format, step["content"].as_str().unwrap_or_default())?
		}
		"run" => doc.run_named(name(step, "command"), args)?,
		"can" => doc.can_named(name(step, "command"), args)?,
		"query" => return doc.query(name(step, "query"), args),
		"undo" => doc.undo()?,
		"redo" => doc.redo()?,
		#[cfg(feature = "collab")]
		operation => return collab_op(doc, operation, step),
		#[cfg(not(feature = "collab"))]
		operation => panic!("unknown op `{operation}`"),
	};

	Ok(Json::Bool(outcome))
}

#[cfg(feature = "collab")]
fn collab_op(doc: &mut Doc, operation: &str, step: &Json) -> Result<Json, HeadlessError> {
	Ok(match operation {
		"share" => {
			doc.share(step["clientId"].as_u64().unwrap_or_default())?;
			Json::Bool(true)
		}
		"applyUpdate" => Json::Bool(doc.apply_update(&base64(&step["update"]))?),
		"takeUpdates" => json!(doc.take_updates()?.len()),
		"validate" => {
			let validation = doc.validate()?;
			json!([validation.rejected_blocks, validation.waiting])
		}
		"receiveSync" => {
			let sync = doc.receive_sync(&base64(&step["message"]))?;
			json!([sync.reply.len(), sync.broadcast.len(), sync.changed])
		}
		unknown => panic!("unknown op `{unknown}`"),
	})
}

#[cfg(feature = "collab")]
fn rejoin(doc: &mut Doc, step: &Json) -> Result<(), String> {
	let state = doc.encode_state().map_err(|error| error.to_string())?;
	*doc = engine()
		.from_yjs(&state, step["clientId"].as_u64().unwrap_or_default())
		.map_err(|error| error.to_string())?;
	Ok(())
}

fn round_trip(doc: &mut Doc) -> Result<(), String> {
	let text = doc.snapshot().to_json().to_string();
	let json: Json = serde_json::from_str(&text).map_err(|error| error.to_string())?;
	let snapshot = Snapshot::from_json(&json).map_err(|error| error.to_string())?;
	*doc = engine()
		.restore(&snapshot)
		.map_err(|error| error.to_string())?;
	Ok(())
}

fn name<'a>(step: &'a Json, key: &str) -> &'a str {
	step[key].as_str().unwrap_or_default()
}

fn position(json: &Json) -> Position {
	let path = json["path"]
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(Json::as_u64)
		.map(|index| index as u32)
		.collect::<Vec<_>>();
	Position::new(path, json["offset"].as_u64().unwrap_or(u64::MAX) as u32)
}

#[cfg(feature = "collab")]
fn base64(json: &Json) -> Vec<u8> {
	use base64::Engine as _;

	base64::engine::general_purpose::STANDARD
		.decode(json.as_str().unwrap_or_default())
		.expect("valid base64 in the fixture")
}

/// Compare `doc` with every expectation in `expect`.
fn check(doc: &Doc, expect: &Json) -> Result<(), String> {
	for format in Format::ALL {
		let Some(expected) = expect.get(format.name()) else {
			continue;
		};

		let (actual, expected) = match format {
			Format::Json => (strip_ids(doc.to_json()), strip_ids(expected.clone())),
			_ => (Json::String(doc.export(format)), expected.clone()),
		};

		if actual != expected {
			return Err(format!(
				"{format} is\n      {actual}\n    expected\n      {expected}"
			));
		}
	}

	if let Some(expected) = expect.get("selection") {
		let actual = doc.selected_range().map_or(
			Json::Null,
			|(anchor, head)| json!({ "anchor": position_json(&anchor), "head": position_json(&head) }),
		);

		if &actual != expected {
			return Err(format!("selection is {actual}, expected {expected}"));
		}
	}

	Ok(())
}

fn position_json(position: &Position) -> Json {
	json!({ "path": position.path, "offset": position.offset })
}

/// Remove `id` from every `attrs` object, then every `attrs` left empty.
fn strip_ids(json: Json) -> Json {
	match json {
		Json::Array(items) => Json::Array(items.into_iter().map(strip_ids).collect()),
		Json::Object(mut fields) => {
			remove_id(&mut fields);
			Json::Object(
				fields
					.into_iter()
					.map(|(key, value)| (key, strip_ids(value)))
					.collect(),
			)
		}
		other => other,
	}
}

fn remove_id(fields: &mut serde_json::Map<String, Json>) {
	let Some(Json::Object(attrs)) = fields.get_mut("attrs") else {
		return;
	};

	attrs.remove("id");

	if attrs.is_empty() {
		fields.remove("attrs");
	}
}
