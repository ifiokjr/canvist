//! Canvas-native extensions: ink drawings and kinetic text, by command and
//! through HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_core::Editor;
use canvist_core::EditorError;
use canvist_core::Env;
use canvist_extensions::InkHandle;
use canvist_extensions::InkStroke;
use canvist_extensions::KineticEffect;
use canvist_extensions::KineticHandle;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Value;
use serde_json::json;
use support::Typist;

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

fn typist(script: &str) -> Typist {
	let mut typist = Typist::with(support::editor_with(canvist_extensions::extras()));
	typist.type_script(script);
	typist
}

fn run(
	editor: &mut Editor,
	name: &str,
	args: &[(&'static str, Value)],
) -> Result<bool, EditorError> {
	let args: Attrs = args.iter().cloned().collect();
	Ok(editor.run_named(name, &args, &mut env())?.is_some())
}

fn floats(values: &[f64]) -> Value {
	Value::from(
		values
			.iter()
			.map(|&value| Value::Float(value))
			.collect::<Vec<_>>(),
	)
}

fn inks(editor: &Editor) -> Value {
	editor.query("inks", &Attrs::new()).expect("a known query")
}

#[test]
fn strokes_are_added_undone_and_cleared_one_step_each() {
	let mut typist = typist("Sketch:");

	assert!(run(&mut typist.editor, "insertInk", &[]).expect("runs"));
	for points in [[10.0, 20.0, 30.0, 40.0], [500.0, 100.0, 520.0, 140.0]] {
		assert!(
			run(
				&mut typist.editor,
				"addInkStroke",
				&[
					("ink", Value::Int(0)),
					("points", floats(&points)),
					("color", Value::from("#2B8A3E"))
				]
			)
			.expect("runs")
		);
	}

	assert_eq!(
		typist.snapshot()["doc"][1]["attrs"]["strokes"][0],
		json!({ "color": "#2b8a3e", "points": [10.0, 20.0, 30.0, 40.0] })
	);
	assert_eq!(
		inks(&typist.editor),
		Value::from(vec![Value::from(
			Attrs::new().with("height", 400).with("strokes", 2)
		)])
	);

	assert!(
		run(
			&mut typist.editor,
			"undoInkStroke",
			&[("ink", Value::Int(0))]
		)
		.expect("runs")
	);
	assert!(run(&mut typist.editor, "clearInk", &[("ink", Value::Int(0))]).expect("runs"));
	assert!(
		!run(&mut typist.editor, "clearInk", &[("ink", Value::Int(0))]).expect("runs"),
		"nothing to clear"
	);
	assert!(
		!run(&mut typist.editor, "clearInk", &[("ink", Value::Int(3))]).expect("runs"),
		"no such drawing"
	);

	typist.unwind(false);
	assert_eq!(
		inks(&typist.editor),
		Value::from(Vec::<Value>::new()),
		"every step undoes"
	);
}

#[test]
fn bad_strokes_and_sizes_are_rejected() {
	let mut typist = typist("");
	run(&mut typist.editor, "insertInk", &[]).expect("runs");

	for args in [
		vec![("ink", Value::Int(0)), ("points", floats(&[1.0, 2.0, 3.0]))],
		vec![
			("ink", Value::Int(0)),
			("points", floats(&[1.0, 2.0])),
			("color", Value::from("red")),
		],
		vec![
			("ink", Value::Int(0)),
			("points", floats(&[1.0, 2.0])),
			("width", Value::Float(-1.0)),
		],
	] {
		assert!(matches!(
			run(&mut typist.editor, "addInkStroke", &args),
			Err(EditorError::InvalidArgs { .. })
		));
	}

	assert!(
		run(
			&mut typist.editor,
			"insertInk",
			&[("height", Value::Int(0))]
		)
		.is_err()
	);
}

#[test]
fn drawings_round_trip_through_html_and_markdown() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let html = "<figure data-type=\"ink\" data-height=\"300\" data-strokes=\"[{&quot;points&quot;:[1.5,2.0,30.0,40.0],&quot;width&quot;:6.0}]\" role=\"img\" aria-label=\"Drawing\"><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 300\"><path d=\"M1.5 2L30 40\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"6\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg></figure>";
	let slice = clipboard::from_html(schema, html);

	assert_eq!(clipboard::to_html(schema, &slice), html);

	let markdown = clipboard::to_markdown(schema, &slice);
	assert_eq!(markdown, html, "Markdown carries a drawing as its HTML");
	assert_eq!(
		clipboard::to_html(schema, &clipboard::from_markdown(schema, &markdown)),
		html
	);
}

#[test]
fn kinetic_text_is_set_toggled_and_read() {
	let mut typist = typist("Say hello[Shift-ArrowLeft][Shift-ArrowLeft][Shift-ArrowLeft][Shift-ArrowLeft][Shift-ArrowLeft]");

	assert!(
		run(
			&mut typist.editor,
			"setKinetic",
			&[("effect", Value::from("rainbow"))]
		)
		.expect("runs")
	);
	assert_eq!(
		typist
			.editor
			.query("kineticEffect", &Attrs::new())
			.expect("a query"),
		Value::from("rainbow")
	);
	assert_eq!(
		typist.snapshot()["doc"][0]["content"][1]["marks"][0],
		json!({ "type": "kinetic", "attrs": { "effect": "rainbow" } })
	);

	assert!(
		run(
			&mut typist.editor,
			"toggleKinetic",
			&[("effect", Value::from("rainbow"))]
		)
		.expect("runs")
	);
	assert_eq!(
		typist
			.editor
			.query("kineticEffect", &Attrs::new())
			.expect("a query"),
		Value::Null
	);
	assert!(
		run(
			&mut typist.editor,
			"setKinetic",
			&[("effect", Value::from("spin"))]
		)
		.is_err()
	);
}

#[test]
fn kinetic_text_round_trips_through_html_and_is_plain_in_markdown() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let html = "<p>Say <span data-kinetic=\"wave\">hello</span></p>";
	let slice = clipboard::from_html(schema, html);

	assert_eq!(clipboard::to_html(schema, &slice), html);
	assert_eq!(clipboard::to_markdown(schema, &slice), "Say hello");
}

/// The canvas-native extensions' typed handles, on a StarterKit editor
/// holding `doc` (ProseMirror JSON) or one empty paragraph.
fn handles(doc: Option<&serde_json::Value>) -> (Typist, InkHandle, KineticHandle) {
	let mut builder = Editor::builder();
	canvist_extensions::StarterKit::add_all(&mut builder);
	let ink = builder.add(canvist_extensions::Ink);
	let kinetic = builder.add(canvist_extensions::Kinetic);
	let editor = builder
		.client(1)
		.build_with(|schema, client| {
			Ok(match doc {
				Some(json) => canvist_model::Document::from_json(schema, client, json)?,
				None => canvist_model::Document::new(schema, client),
			})
		})
		.expect("the canvas-native extensions build");

	(Typist::with(editor), ink, kinetic)
}

fn apply(typist: &mut Typist, command: &canvist_core::Command) -> bool {
	typist
		.editor
		.run(command, &mut env())
		.expect("commands run")
		.is_some()
}

fn can(typist: &mut Typist, command: &canvist_core::Command) -> bool {
	typist
		.editor
		.can(command, &mut env())
		.expect("dry runs run")
}

fn stroke(points: &[f32]) -> InkStroke<'_> {
	InkStroke {
		points,
		color: None,
		width: None,
	}
}

#[test]
fn the_ink_handle_draws_validates_and_undoes_by_block() {
	let (mut typist, ink, _) = handles(None);
	assert_eq!(ink.drawings(typist.editor.state()), []);
	assert!(apply(&mut typist, &ink.insert(300)));
	assert!(apply(&mut typist, &ink.insert(400)));
	let drawings = ink.drawings(typist.editor.state());
	assert_eq!(drawings.len(), 2);
	assert_eq!(
		typist.snapshot()["doc"][0]["attrs"],
		json!({ "height": 300 }),
		"a non-default height is stored, the default is not"
	);

	for bad in [
		stroke(&[]),
		stroke(&[1.0]),
		stroke(&[1.0, f32::INFINITY]),
		InkStroke {
			color: Some("#abcdeg"),
			..stroke(&[1.0, 2.0])
		},
		InkStroke {
			width: Some(f32::NAN),
			..stroke(&[1.0, 2.0])
		},
	] {
		assert!(
			matches!(
				ink.add_stroke(drawings[1], &bad),
				Err(EditorError::InvalidArgs { .. })
			),
			"{bad:?} is refused"
		);
	}

	let add = ink
		.add_stroke(
			drawings[1],
			&InkStroke {
				color: Some("#ABCDEF"),
				width: Some(2.25),
				..stroke(&[0.04, 0.0, 999.96, 4.0])
			},
		)
		.expect("a valid stroke");
	assert!(can(&mut typist, &add));
	assert!(apply(&mut typist, &add));
	assert_eq!(
		typist.snapshot()["doc"][1]["attrs"]["strokes"],
		json!([{ "points": [0.0, 0.0, 1000.0, 4.0], "color": "#abcdef", "width": 2.25 }])
	);
	let paragraph = typist.editor.state().doc().children(None)[2];
	let onto_paragraph = ink
		.add_stroke(paragraph, &stroke(&[1.0, 2.0]))
		.expect("valid");
	assert!(
		!apply(&mut typist, &onto_paragraph),
		"a paragraph is not a drawing"
	);

	assert!(apply(&mut typist, &ink.undo_stroke(drawings[1])));
	assert!(!can(&mut typist, &ink.undo_stroke(drawings[1])));
	assert!(!apply(&mut typist, &ink.clear(drawings[1])));
	typist.editor.undo(&mut env()).expect("undo applies");
	assert!(apply(&mut typist, &ink.clear(drawings[1])));
	typist.editor.undo(&mut env()).expect("undo applies");
	assert_eq!(
		typist.snapshot()["doc"][1]["attrs"]["strokes"][0]["width"],
		2.25,
		"undoing a clear brings the stroke back exactly"
	);
}

#[test]
fn the_kinetic_handle_sets_toggles_and_reads_every_effect() {
	let (mut typist, _, kinetic) = handles(Some(&json!({
		"type": "doc",
		"content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "move me" }] }]
	})));
	typist.type_script("[Mod-a]");

	assert_eq!(kinetic.effect(typist.editor.state()), None);

	for effect in KineticEffect::ALL {
		assert!(apply(&mut typist, &kinetic.set(effect)));
		assert_eq!(kinetic.effect(typist.editor.state()), Some(effect));
	}

	assert!(apply(&mut typist, &kinetic.toggle(KineticEffect::Wave)));
	assert_eq!(
		kinetic.effect(typist.editor.state()),
		Some(KineticEffect::Wave),
		"toggling another effect switches to it"
	);
	assert!(apply(&mut typist, &kinetic.toggle(KineticEffect::Wave)));
	assert_eq!(
		kinetic.effect(typist.editor.state()),
		None,
		"toggling the same one stills it"
	);
	assert!(apply(&mut typist, &kinetic.toggle(KineticEffect::Pulse)));
	assert!(apply(&mut typist, &kinetic.unset()));
	assert_eq!(kinetic.effect(typist.editor.state()), None);
	assert_eq!(KineticEffect::default(), KineticEffect::Wave);
	assert_eq!(
		KineticEffect::ALL.map(KineticEffect::name),
		["wave", "shake", "pulse", "rainbow"]
	);
}

/// Regression: toggling an effect over text moving with another stilled it
/// instead of switching, unlike toggling a highlight colour.
#[test]
fn toggling_another_effect_switches_to_it() {
	let mut typist = typist("Say hello[Mod-a]");

	run(
		&mut typist.editor,
		"setKinetic",
		&[("effect", Value::from("pulse"))],
	)
	.expect("runs");
	run(
		&mut typist.editor,
		"toggleKinetic",
		&[("effect", Value::from("shake"))],
	)
	.expect("runs");
	assert_eq!(
		typist
			.editor
			.query("kineticEffect", &Attrs::new())
			.expect("a query"),
		Value::from("shake")
	);
}

/// Regression: a stroke width went from `f32` to `f64` bit for bit, so 3.3
/// was stored as 3.299999952316284 and HTML read back a different number.
#[test]
fn stroke_widths_are_stored_as_written() {
	let mut typist = typist("");
	run(&mut typist.editor, "insertInk", &[]).expect("runs");
	run(
		&mut typist.editor,
		"addInkStroke",
		&[
			("ink", Value::Int(0)),
			("points", floats(&[1.0, 2.0])),
			("width", Value::Float(3.3)),
		],
	)
	.expect("runs");

	assert_eq!(
		typist.snapshot()["doc"][0]["attrs"]["strokes"][0]["width"],
		json!(3.3)
	);
}

/// Regression: a negative drawing index quietly did nothing rather than
/// being refused like other negative indexes.
#[test]
fn negative_drawing_indexes_are_refused() {
	let mut typist = typist("");
	run(&mut typist.editor, "insertInk", &[]).expect("runs");

	for name in ["addInkStroke", "undoInkStroke", "clearInk"] {
		assert!(
			matches!(
				run(
					&mut typist.editor,
					name,
					&[("ink", Value::Int(-1)), ("points", floats(&[1.0, 2.0]))]
				),
				Err(EditorError::InvalidArgs { .. })
			),
			"{name}"
		);
	}
}

/// Regression: a drawing whose stored data was malformed wrote an invalid
/// SVG viewBox and empty paths into HTML and Markdown.
#[test]
fn malformed_stored_drawings_still_write_valid_html() {
	let doc = json!({
		"type": "doc",
		"content": [{
			"type": "ink",
			"attrs": { "height": -5, "strokes": [{ "points": "none" }, { "points": [1, 2, 3, 4] }, 7] }
		}]
	});
	let (mut typist, ink, _) = handles(Some(&doc));
	let schema = typist.editor.schema().clone();
	typist.type_script("[Mod-a]");
	let slice = typist.editor.copy().expect("a copy");
	let html = clipboard::to_html(&schema, &slice);

	assert!(html.contains("viewBox=\"0 0 1000 1\""), "{html}");
	assert!(!html.contains("d=\"\""), "{html}");
	assert_eq!(html.matches("<path").count(), 1, "{html}");
	assert_eq!(ink.drawings(typist.editor.state()).len(), 1);
}

#[test]
fn drawings_are_named_images_in_html() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let slice = clipboard::from_html(
		schema,
		"<figure data-type=\"ink\" data-strokes=\"[]\"><svg></svg></figure>",
	);
	let html = clipboard::to_html(schema, &slice);

	assert!(
		html.starts_with(
			"<figure data-type=\"ink\" data-height=\"400\" data-strokes=\"[]\" role=\"img\" \
			 aria-label=\"Drawing\">"
		),
		"{html}"
	);
}

mod properties {
	use proptest::prelude::*;

	use super::*;

	fn points() -> impl Strategy<Value = Vec<f32>> {
		prop::collection::vec((-50.0_f32..1100.0, -50.0_f32..5000.0), 1..8)
			.prop_map(|pairs| pairs.into_iter().flat_map(|(x, y)| [x, y]).collect())
	}

	fn color() -> impl Strategy<Value = Option<String>> {
		prop::option::of("#[0-9a-fA-F]{6}")
	}

	fn effect() -> impl Strategy<Value = KineticEffect> {
		prop::sample::select(KineticEffect::ALL.to_vec())
	}

	proptest! {
		#![proptest_config(ProptestConfig::with_cases(96))]

		#[test]
		fn strokes_round_trip_through_html_markdown_json_and_snapshots(
			strokes in prop::collection::vec((points(), color(), prop::option::of(0.5_f32..40.0)), 1..4),
			height in 1_i64..4000,
		) {
			let (mut typist, ink, _) = handles(None);
			prop_assert!(apply(&mut typist, &ink.insert(height)));
			let drawing = ink.drawings(typist.editor.state())[0];

			for (points, color, width) in &strokes {
				let stroke = InkStroke { points, color: color.as_deref(), width: *width };
				prop_assert!(apply(&mut typist, &ink.add_stroke(drawing, &stroke).expect("valid")));
			}

			let schema = typist.editor.schema().clone();
			let stored = typist.snapshot()["doc"].clone();
			let saved = &stored[0]["attrs"]["strokes"];

			for (index, (points, color, _)) in strokes.iter().enumerate() {
				// x stays on the page and y below its top, to a tenth of a unit.
				let kept: Vec<f64> = points
					.iter()
					.enumerate()
					.map(|(at, &point)| if at % 2 == 0 { point.clamp(0.0, 1000.0) } else { point.max(0.0) })
					.map(|point| f64::from((point * 10.0).round()) / 10.0)
					.collect();
				prop_assert_eq!(&saved[index]["points"], &json!(kept));
				prop_assert_eq!(
					&saved[index]["color"],
					&color.as_ref().map_or(serde_json::Value::Null, |color| json!(color.to_ascii_lowercase()))
				);
			}

			typist.type_script("[Mod-a]");
			let slice = typist.editor.copy().expect("a copy");
			let html = clipboard::to_html(&schema, &slice);
			let markdown = clipboard::to_markdown(&schema, &slice);
			let from_html = clipboard::from_html(&schema, &html);
			let from_markdown = clipboard::from_markdown(&schema, &markdown);

			prop_assert_eq!(clipboard::to_html(&schema, &from_html), html.clone());
			// Markdown has no empty paragraphs, so the trailing one is left out.
			prop_assert_eq!(
				clipboard::to_html(&schema, &from_markdown),
				html.trim_end_matches("<p></p>")
			);

			let json = typist.editor.state().doc().to_json();
			let restored = canvist_model::Document::from_json(schema.clone(), 1, &json).expect("its own JSON");
			prop_assert_eq!(restored.to_json(), json);

			typist.editor.undo(&mut env()).expect("undo applies");
			prop_assert_eq!(
				typist.snapshot()["doc"][0]["attrs"]["strokes"].as_array().map(Vec::len).unwrap_or_default(),
				strokes.len() - 1,
				"each stroke is its own step"
			);
		}

		#[test]
		fn kinetic_marks_round_trip_through_html(
			text in "[a-z]{1,4}( [a-zé]{1,4}){0,2}",
			start in 0_usize..12,
			len in 1_usize..12,
			effect in effect(),
		) {
			let (mut typist, _, kinetic) = handles(Some(&json!({
				"type": "doc",
				"content": [{ "type": "paragraph", "content": [{ "type": "text", "text": text }] }]
			})));
			let block = typist.editor.state().doc().children(None)[0];
			// Character positions, as byte offsets.
			let bounds: Vec<usize> = text
				.char_indices()
				.map(|(offset, _)| offset)
				.chain([text.len()])
				.collect();
			let first = start.min(bounds.len() - 2);
			let start = bounds[first];
			let end = bounds[(first + len).min(bounds.len() - 1)];
			typist
				.editor
				.select(
					canvist_model::Selection::text(
						canvist_model::Point::new(block, start as u32),
						canvist_model::Point::new(block, end as u32),
					),
					&mut env(),
				)
				.expect("a selection");
			prop_assert!(apply(&mut typist, &kinetic.set(effect)));
			prop_assert_eq!(kinetic.effect(typist.editor.state()), Some(effect));

			let schema = typist.editor.schema().clone();
			typist.type_script("[Mod-a]");
			let slice = typist.editor.copy().expect("a copy");
			let html = clipboard::to_html(&schema, &slice);
			prop_assert!(html.contains(&format!("data-kinetic=\"{}\"", effect.name())), "{}", html);
			prop_assert_eq!(clipboard::to_html(&schema, &clipboard::from_html(&schema, &html)), html);
			let markdown = clipboard::to_markdown(&schema, &slice);
			prop_assert_eq!(markdown.trim_end(), text.trim_end());
		}
	}
}
