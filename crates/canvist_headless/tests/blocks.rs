//! Details, callouts, and math through the headless API: Yjs peers editing
//! them concurrently converge, JSON and Yjs carry their attributes, stable
//! error codes, and the accessible markup of server-rendered HTML.

#![cfg(feature = "extras")]

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::ErrorCode;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use serde_json::Value as Json;
use serde_json::json;

const BLOCKS: [&str; 4] = ["history", "details", "callout", "mathematics"];

fn engine(client: u32) -> Engine {
	Engine::named(&BLOCKS)
		.expect("the block extensions build")
		.with_client(client)
}

fn caret(doc: &mut Doc, path: &[u32], offset: u32) -> Result<(), HeadlessError> {
	let position = Position::new(path.to_vec(), offset);
	doc.select_range(&position, &position)
}

#[test]
fn tiptap_json_with_unknown_or_missing_attributes_reads_as_the_defaults()
-> Result<(), HeadlessError> {
	let mut doc = engine(1).from_json(&json!({ "type": "doc", "content": [
		{ "type": "callout", "attrs": { "type": "purple" }, "content": [
			{ "type": "paragraph", "content": [{ "type": "text", "text": "a" }] },
		] },
		{ "type": "callout", "content": [
			{ "type": "paragraph", "content": [{ "type": "text", "text": "b" }] },
		] },
		{ "type": "details", "content": [
			{ "type": "detailsSummary", "content": [{ "type": "text", "text": "s" }] },
			{ "type": "detailsContent", "content": [{ "type": "paragraph" }] },
		] },
	] }))?;

	caret(&mut doc, &[0, 0], 0)?;
	assert_eq!(doc.query("calloutType", &Json::Null)?, json!("info"));
	caret(&mut doc, &[1, 0], 0)?;
	assert_eq!(doc.query("calloutType", &Json::Null)?, json!("info"));
	caret(&mut doc, &[2, 0], 0)?;
	assert_eq!(doc.query("detailsOpen", &Json::Null)?, json!(false));
	assert_eq!(
		doc.export(Format::Markdown),
		"> [!NOTE]\n> a\n\n> [!NOTE]\n> b\n\n<details>\n<summary>s</summary>\n\n\n\n</details>"
	);
	Ok(())
}

#[test]
fn errors_have_stable_codes() -> Result<(), HeadlessError> {
	let mut doc = engine(1).parse(Format::Markdown, "x")?;
	let code = |result: Result<bool, HeadlessError>| result.map_err(|error| error.code()).err();

	assert_eq!(
		code(doc.run_named("toggleCallout", &json!({ "type": "purple" }))),
		Some(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.run_named("insertInlineMath", &json!({}))),
		Some(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.run_named("insertBlockMath", &json!({ "latex": 2 }))),
		Some(ErrorCode::InvalidArgs)
	);
	assert_eq!(
		code(doc.can_named("setCalloutType", &json!({}))),
		Some(ErrorCode::InvalidArgs)
	);
	assert_eq!(doc.export(Format::Markdown), "x");

	let mut plain = Engine::named(&["history"])?.parse(Format::Markdown, "x")?;
	for name in ["setDetails", "toggleCallout", "insertBlockMath"] {
		assert_eq!(
			code(plain.run_named(name, &Json::Null)),
			Some(ErrorCode::UnknownName),
			"{name}"
		);
	}
	assert_eq!(
		plain
			.query("calloutType", &Json::Null)
			.map_err(|error| error.code())
			.err(),
		Some(ErrorCode::UnknownName)
	);
	Ok(())
}

#[cfg(feature = "collab")]
mod collab {
	use super::*;

	/// Two peers sharing `markdown`, the second joined from the first's
	/// state.
	fn peers(markdown: &str) -> Result<(Doc, Doc), HeadlessError> {
		let mut first = engine(1).parse(Format::Markdown, markdown)?;
		first.share(1)?;
		let second = engine(2).from_yjs(&first.encode_state()?, 2)?;
		assert_eq!(second.to_json(), first.to_json(), "the second peer joins");
		Ok((first, second))
	}

	/// Deliver every update each peer made to the other.
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

	/// The peers hold the same document, which a third peer joining from
	/// the shared state also reads, and nothing is rejected.
	fn assert_converged(first: &Doc, second: &Doc) -> Result<(), HeadlessError> {
		assert_eq!(first.to_json(), second.to_json(), "the peers converge");
		let third = engine(3).from_yjs(&first.encode_state()?, 3)?;
		assert_eq!(
			third.to_json(),
			first.to_json(),
			"the shared state holds it"
		);
		assert!(first.validate()?.is_valid());
		assert!(second.validate()?.is_valid());
		Ok(())
	}

	#[test]
	fn closing_details_while_another_peer_writes_in_it() -> Result<(), HeadlessError> {
		let (mut first, mut second) =
			peers("<details open>\n<summary>Sum</summary>\n\nBody\n\n</details>")?;

		caret(&mut first, &[0, 0], 0)?;
		assert!(first.run_named("toggleDetailsOpen", &Json::Null)?);
		caret(&mut second, &[0, 0], 3)?;
		second.insert_text("mary")?;
		caret(&mut second, &[0, 1, 0], 4)?;
		second.insert_text(" text")?;
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		assert_eq!(
			first.export(Format::Markdown),
			"<details>\n<summary>Summary</summary>\n\nBody text\n\n</details>"
		);
		Ok(())
	}

	#[test]
	fn changing_a_callout_type_while_another_peer_types_in_it() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("> [!NOTE]\n> Careful")?;

		caret(&mut first, &[0, 0], 0)?;
		assert!(first.run_named("setCalloutType", &json!({ "type": "warning" }))?);
		caret(&mut second, &[0, 0], 7)?;
		second.insert_text(" now")?;
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		assert_eq!(
			first.export(Format::Markdown),
			"> [!WARNING]\n> Careful now"
		);
		assert_eq!(second.query("calloutType", &Json::Null)?, json!("warning"));
		Ok(())
	}

	#[test]
	fn both_peers_inserting_formulas_keeps_both() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("Area and volume")?;

		caret(&mut first, &[0], 4)?;
		assert!(first.run_named("insertInlineMath", &json!({ "latex": "\\pi r^2" }))?);
		caret(&mut second, &[0], 15)?;
		assert!(second.run_named("insertBlockMath", &json!({ "latex": "V = s^3" }))?);
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		assert_eq!(
			first.export(Format::Markdown),
			"Area$\\pi r^2$ and volume\n\n$$\nV = s^3\n$$"
		);
		Ok(())
	}

	#[test]
	fn wrapping_a_block_while_another_peer_edits_it() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("One\n\nTwo")?;

		caret(&mut first, &[0], 0)?;
		assert!(first.run_named("toggleCallout", &json!({ "type": "success" }))?);
		caret(&mut second, &[1], 3)?;
		second.insert_text("!")?;
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		assert_eq!(first.export(Format::Markdown), "> [!TIP]\n> One\n\nTwo!");

		caret(&mut first, &[1], 0)?;
		assert!(first.run_named("setDetails", &Json::Null)?);
		first.insert_text("Hidden")?;
		caret(&mut second, &[0, 0], 3)?;
		second.insert_text("?")?;
		exchange(&mut first, &mut second)?;

		assert_converged(&first, &second)?;
		assert_eq!(
			first.export(Format::Markdown),
			"> [!TIP]\n> One?\n\n<details open>\n<summary>Hidden</summary>\n\nTwo!\n\n</details>"
		);
		Ok(())
	}

	#[test]
	fn undoing_a_block_command_reaches_the_other_peer() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("Plain")?;
		let original = first.to_json();

		caret(&mut first, &[0], 0)?;
		assert!(first.run_named("setDetails", &Json::Null)?);
		exchange(&mut first, &mut second)?;
		// The second peer's caret was in the wrapped paragraph, so it is in
		// the details now.
		assert_eq!(second.query("detailsOpen", &Json::Null)?, json!(true));
		assert_converged(&first, &second)?;

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

	fn html(markdown: &str) -> String {
		let doc = engine(1)
			.parse(Format::Markdown, markdown)
			.expect("Markdown reads");
		Renderer::new().html(&doc, 480.0).html
	}

	#[test]
	fn callouts_are_notes_labelled_with_their_type() {
		for (alert, label) in [
			("NOTE", Some("Info")),
			("TIP", Some("Success")),
			("WARNING", Some("Warning")),
			("CAUTION", Some("Error")),
			("IMPORTANT", None),
		] {
			let html = html(&format!("> [!{alert}]\n> Text"));
			let opening = html
				.split('>')
				.find(|tag| tag.contains("cv-t-callout"))
				.unwrap_or_else(|| panic!("a callout in {html}"));

			assert!(opening.contains(" role=\"note\""), "{opening}");
			match label {
				Some(label) => {
					assert!(
						opening.contains(&format!(" aria-label=\"{label}\"")),
						"{opening}"
					);
				}
				None => assert!(!opening.contains("aria-label"), "{opening}"),
			}
		}
	}

	#[test]
	fn formulas_carry_their_latex_as_accessible_text() {
		let html = html("Area $\\pi r^2$\n\n$$\nx < \"y\"\n$$");

		assert!(html.contains(">\\pi r^2</span>"), "{html}");
		assert!(
			html.contains(" role=\"math\" aria-label=\"x &lt; &quot;y&quot;\""),
			"{html}"
		);
	}

	#[test]
	fn closed_details_render_only_their_summary() {
		let closed = html("<details>\n<summary>Sum</summary>\n\nHidden\n\n</details>");
		let open = html("<details open>\n<summary>Sum</summary>\n\nHidden\n\n</details>");

		assert!(
			closed.contains("Sum") && !closed.contains("Hidden"),
			"{closed}"
		);
		assert!(closed.contains("cv-mk-d0"), "{closed}");
		assert!(
			open.contains("Hidden") && open.contains("cv-mk-d1"),
			"{open}"
		);
	}
}
