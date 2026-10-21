//! Tables through the headless API: Yjs peers changing a table's shape
//! while others type in it converge, TipTap JSON with merged cells, stable
//! error codes, and the table semantics of server-rendered HTML.

#![cfg(feature = "extras")]

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::ErrorCode;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use serde_json::Value as Json;
use serde_json::json;

const NAMES: [&str; 4] = ["history", "bold", "textAlign", "table"];

fn engine(client: u32) -> Engine {
	Engine::named(&NAMES)
		.expect("the table extension builds")
		.with_client(client)
}

fn caret(doc: &mut Doc, path: &[u32], offset: u32) -> Result<(), HeadlessError> {
	let position = Position::new(path.to_vec(), offset);
	doc.select_range(&position, &position)
}

/// The text of every cell of the first table, row by row.
fn cells(doc: &Doc) -> Vec<Vec<String>> {
	let json = doc.to_json();
	let table = json["content"]
		.as_array()
		.and_then(|blocks| blocks.iter().find(|block| block["type"] == "table"))
		.cloned()
		.unwrap_or(Json::Null);
	let text = |cell: &Json| -> String {
		cell["content"]
			.as_array()
			.into_iter()
			.flatten()
			.flat_map(|paragraph| paragraph["content"].as_array().cloned().unwrap_or_default())
			.filter_map(|run| run["text"].as_str().map(str::to_owned))
			.collect()
	};

	table["content"]
		.as_array()
		.into_iter()
		.flatten()
		.map(|row| {
			row["content"]
				.as_array()
				.into_iter()
				.flatten()
				.map(text)
				.collect()
		})
		.collect()
}

#[test]
fn tiptap_json_with_merged_cells_reads_and_answers_the_cell_query() -> Result<(), HeadlessError> {
	let cell = |kind: &str, attrs: Json, text: &str| {
		json!({
			"type": kind,
			"attrs": attrs,
			"content": [{ "type": "paragraph", "content": [{ "type": "text", "text": text }] }],
		})
	};
	let mut doc = engine(1).from_json(
		&json!({ "type": "doc", "content": [{ "type": "table", "content": [
		{ "type": "tableRow", "content": [
			cell("tableHeader", json!({ "colspan": 2, "colwidth": [120, 80] }), "Wide"),
		] },
		{ "type": "tableRow", "content": [
			cell("tableCell", json!({ "rowspan": 2 }), "Tall"),
			cell("tableCell", json!({}), "b"),
		] },
		{ "type": "tableRow", "content": [cell("tableCell", json!({}), "c")] },
	] }] }),
	)?;

	caret(&mut doc, &[0, 2, 0, 0], 0)?;
	assert_eq!(
		doc.query("tableCell", &Json::Null)?,
		json!({ "row": 2, "column": 1, "rowspan": 1, "colspan": 1, "header": false, "rows": 3, "columns": 2 })
	);
	caret(&mut doc, &[0, 0, 0, 0], 0)?;
	assert_eq!(doc.query("tableCell", &Json::Null)?["colspan"], 2);
	assert_eq!(
		doc.export(Format::Html),
		"<table><tbody><tr><th colspan=\"2\" colwidth=\"120,80\"><p>Wide</p></th></tr><tr><td \
		 rowspan=\"2\"><p>Tall</p></td><td><p>b</p></td></tr><tr><td><p>c</p></td></tr></tbody></\
		 table>"
	);
	Ok(())
}

#[test]
fn errors_have_stable_codes() -> Result<(), HeadlessError> {
	let mut doc = engine(1).parse(Format::Markdown, "x")?;
	let code = |result: Result<bool, HeadlessError>| result.map_err(|error| error.code()).err();

	assert_eq!(
		code(doc.run_named("insertTable", &json!({ "rows": 0 }))),
		Some(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.run_named("insertTable", &json!({ "cols": "3" }))),
		Some(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.run_named("setCellAttribute", &json!({ "name": "x" }))),
		Some(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.can_named("addRowAfter", &json!({ "count": 1 }))),
		Some(ErrorCode::InvalidArgs)
	);
	assert!(
		!doc.run_named("addRowAfter", &Json::Null)?,
		"outside a table"
	);
	assert!(!doc.run_named("mergeCells", &Json::Null)?);
	assert_eq!(doc.query("tableCell", &Json::Null)?, Json::Null);
	assert_eq!(doc.export(Format::Markdown), "x");

	let mut plain = Engine::named(&["history"])?.parse(Format::Markdown, "x")?;
	for name in ["insertTable", "addRowAfter", "mergeCells", "goToNextCell"] {
		assert_eq!(
			code(plain.run_named(name, &Json::Null)),
			Some(ErrorCode::UnknownName),
			"{name}"
		);
	}
	assert_eq!(
		plain
			.query("tableCell", &Json::Null)
			.map_err(|error| error.code())
			.err(),
		Some(ErrorCode::UnknownName)
	);
	Ok(())
}

#[cfg(feature = "collab")]
mod collab {
	use super::*;

	const TABLE: &str = "| a | b |\n| --- | --- |\n| c | d |";

	fn peers(markdown: &str) -> Result<(Doc, Doc), HeadlessError> {
		let mut first = engine(1).parse(Format::Markdown, markdown)?;
		first.share(1)?;
		let second = engine(2).from_yjs(&first.encode_state()?, 2)?;
		assert_eq!(second.to_json(), first.to_json(), "the second peer joins");
		Ok((first, second))
	}

	fn exchange(first: &mut Doc, second: &mut Doc) -> Result<(), HeadlessError> {
		let from_first = first.take_updates()?;
		let from_second = second.take_updates()?;

		for update in from_second {
			first.apply_update(&update)?;
		}

		for update in from_first {
			second.apply_update(&update)?;
		}

		Ok(())
	}

	fn assert_converged(first: &Doc, second: &Doc) -> Result<(), HeadlessError> {
		assert_eq!(first.to_json(), second.to_json(), "the peers converge");
		let third = engine(3).from_yjs(&first.encode_state()?, 3)?;
		assert_eq!(
			third.to_json(),
			first.to_json(),
			"the shared state holds it"
		);
		assert!(first.validate()?.is_valid());
		Ok(())
	}

	#[test]
	fn adding_a_row_while_another_peer_types_in_a_cell() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers(TABLE)?;

		caret(&mut first, &[0, 0, 0, 0], 0)?;
		assert!(first.run_named("addRowAfter", &Json::Null)?);
		caret(&mut second, &[0, 1, 1, 0], 1)?;
		second.insert_text("!")?;
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		assert_eq!(
			cells(&first),
			[vec!["a", "b"], vec!["", ""], vec!["c", "d!"]]
		);
		Ok(())
	}

	#[test]
	fn adding_a_column_while_another_peer_adds_a_row() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers(TABLE)?;

		caret(&mut first, &[0, 0, 0, 0], 0)?;
		assert!(first.run_named("addColumnAfter", &Json::Null)?);
		caret(&mut second, &[0, 1, 0, 0], 0)?;
		assert!(second.run_named("addRowAfter", &Json::Null)?);
		exchange(&mut first, &mut second)?;

		assert_eq!(first.to_json(), second.to_json(), "the peers converge");
		// Each peer only saw its own change, so the new row is a column
		// short; the next local edit on either peer pads it.
		assert_eq!(
			cells(&first).iter().map(Vec::len).collect::<Vec<_>>(),
			[3, 3, 2]
		);
		assert!(first.run_named("fixTables", &Json::Null)?);
		exchange(&mut first, &mut second)?;
		assert_converged(&first, &second)?;
		assert_eq!(
			cells(&second).iter().map(Vec::len).collect::<Vec<_>>(),
			[3, 3, 3]
		);
		Ok(())
	}

	#[test]
	fn merging_cells_while_another_peer_types_in_one() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers(TABLE)?;

		first.select_range(
			&Position::new(vec![0, 1, 0, 0], 0),
			&Position::new(vec![0, 1, 1, 0], 0),
		)?;
		assert!(first.run_named("mergeCells", &Json::Null)?);
		// Typing in the cell the others merge into survives. Typing in a cell
		// whose content moves does not: Yjs has no move, so the shared state
		// copies the moved paragraph and drops the original with the typing.
		caret(&mut second, &[0, 1, 0, 0], 1)?;
		second.insert_text("2")?;
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		assert_eq!(cells(&first), [vec!["a", "b"], vec!["c2d"]]);
		assert_eq!(
			first.to_json()["content"][0]["content"][1]["content"][0]["attrs"]["colspan"],
			2
		);
		Ok(())
	}

	#[test]
	fn both_peers_toggling_headers_and_typing_converge() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers(TABLE)?;

		caret(&mut first, &[0, 1, 0, 0], 1)?;
		assert!(first.run_named("toggleHeaderColumn", &Json::Null)?);
		caret(&mut second, &[0, 0, 1, 0], 1)?;
		second.insert_text("eta")?;
		assert!(second.run_named(
			"setCellAttribute",
			&json!({ "name": "align", "value": "right" })
		)?);
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		let json = first.to_json();
		assert_eq!(
			json["content"][0]["content"][1]["content"][0]["type"],
			"tableHeader"
		);
		assert_eq!(
			json["content"][0]["content"][0]["content"][1]["attrs"]["align"],
			"right"
		);
		assert_eq!(cells(&first)[0], ["a", "beta"]);
		Ok(())
	}

	#[test]
	fn undoing_a_table_command_reaches_the_other_peer() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers(TABLE)?;
		let original = second.to_json();

		caret(&mut first, &[0, 0, 0, 0], 0)?;
		assert!(first.run_named("deleteColumn", &Json::Null)?);
		exchange(&mut first, &mut second)?;
		assert_eq!(cells(&second), [vec!["b"], vec!["d"]]);

		assert!(first.undo()?);
		exchange(&mut first, &mut second)?;
		assert_converged(&first, &second)?;
		assert_eq!(second.to_json(), original);
		Ok(())
	}
}

#[cfg(feature = "ssr")]
mod ssr {
	use canvist_headless::Renderer;

	use super::*;

	#[test]
	fn rendered_tables_keep_table_semantics_and_header_scopes() -> Result<(), HeadlessError> {
		let doc = engine(1).parse(
			Format::Html,
			"<table><tr><th>Name</th><th>Score</th></tr><tr><th>Ada</th><td>10</td></tr></table>",
		)?;
		let html = Renderer::new().html(&doc, 480.0).html;

		assert!(html.contains("<table "), "{html}");
		assert!(html.contains("<tbody>") && html.contains("<tr "), "{html}");
		assert_eq!(html.matches(" scope=\"col\"").count(), 2, "{html}");
		assert_eq!(html.matches(" scope=\"row\"").count(), 1, "{html}");
		assert!(html.contains("<td "), "{html}");
		Ok(())
	}

	#[test]
	fn rendered_merged_cells_carry_their_spans() -> Result<(), HeadlessError> {
		let doc = engine(1).parse(
			Format::Html,
			"<table><tr><th colspan=\"2\">Wide</th></tr><tr><td \
			 rowspan=\"2\">Tall</td><td>b</td></tr><tr><td>c</td></tr></table>",
		)?;
		let html = Renderer::new().html(&doc, 480.0).html;

		assert!(html.contains(" scope=\"col\" colspan=\"2\""), "{html}");
		assert!(html.contains(" rowspan=\"2\""), "{html}");
		Ok(())
	}
}
