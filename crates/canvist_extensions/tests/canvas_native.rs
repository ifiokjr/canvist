//! Canvas-native extensions: ink drawings and kinetic text, by command and
//! through HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::EditorError;
use canvist_editor::Env;
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
	let html = "<figure data-type=\"ink\" data-height=\"300\" data-strokes=\"[{&quot;points&quot;:[1.5,2.0,30.0,40.0],&quot;width&quot;:6.0}]\"><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 300\"><path d=\"M1.5 2L30 40\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"6\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg></figure>";
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
