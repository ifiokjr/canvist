//! Behaviour of each step, its inverse, and its point mapping.

mod support;

use canvist_model::Attrs;
use canvist_model::Bias;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::Embed;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Step;
use canvist_model::StepError;
use serde_json::json;

fn first(doc: &Document) -> BlockId {
	doc.first_text_block().expect("document has a text block")
}

fn undo(doc: &mut Document, commit: &canvist_model::Commit) {
	doc.apply_steps(commit.undo_steps()).expect("undo applies");
}

#[test]
fn typing_maps_carets_after_the_insertion() {
	let mut doc = support::paragraphs(1, "held");
	let block = first(&doc);
	let caret = Point::new(block, 3);

	let mut transaction = doc.transaction();
	let end = transaction
		.insert_text(caret, "lo wor", MarkSet::empty())
		.expect("valid point");
	let commit = transaction.commit();

	assert_eq!(doc.plain_text(), "hello word");
	assert_eq!(commit.map(caret, Bias::After).point, end);
	assert_eq!(commit.map(caret, Bias::Before).point, caret);
	assert_eq!(commit.touched, [block]);
	assert!(!commit.structural);
}

#[test]
fn deleting_maps_points_inside_the_range_to_its_start() {
	let mut doc = support::paragraphs(1, "hello world");
	let block = first(&doc);

	let mut transaction = doc.transaction();
	transaction.delete_text(block, 2..8).expect("valid range");
	let commit = transaction.commit();

	let inside = commit.map(Point::new(block, 5), Bias::After);
	assert_eq!(doc.plain_text(), "herld");
	assert_eq!(inside.point, Point::new(block, 2));
	assert!(inside.deleted);
	assert_eq!(
		commit.map(Point::new(block, 10), Bias::After).point,
		Point::new(block, 4)
	);
}

#[test]
fn every_step_kind_round_trips_through_its_inverse() {
	let mut doc = support::paragraphs(3, "abc def");
	let original = doc.to_json();
	let ids: Vec<_> = doc.children(None).to_vec();
	let bold = Mark::new(doc.schema().mark_type("bold").expect("bold"));
	let heading = support::kind(&doc, "heading");
	let quote = support::kind(&doc, "blockquote");
	let paragraph = support::kind(&doc, "paragraph");

	let mut transaction = doc.transaction();
	transaction.add_mark(ids[0], 0..3, bold).expect("format");
	transaction
		.set_block(ids[1], heading, Attrs::new().with("level", 2))
		.expect("set block");
	let tail = transaction
		.split(Point::new(ids[2], 3), paragraph, Attrs::new())
		.expect("split");
	transaction.delete_text(tail.block, 0..1).expect("delete");
	transaction
		.insert_blocks(
			None,
			0,
			vec![BlockTree::container(BlockId::new(5, 0), quote, Vec::new())],
		)
		.expect("insert quote");
	transaction
		.move_blocks((None, 1, 2), (Some(BlockId::new(5, 0)), 0))
		.expect("move into quote");
	transaction.join(ids[2], tail.block).expect("join");

	transaction
		.remove_blocks(Some(BlockId::new(5, 0)), 1, 1)
		.expect("remove");
	let commit = transaction.commit();
	support::assert_invariants(&doc);

	undo(&mut doc, &commit);
	support::assert_invariants(&doc);
	assert_eq!(doc.to_json(), original);
}

#[test]
fn split_and_join_map_points_between_blocks() {
	let mut doc = support::paragraphs(1, "hello world");
	let block = first(&doc);
	let paragraph = support::kind(&doc, "paragraph");

	let mut transaction = doc.transaction();
	let start = transaction
		.split(Point::new(block, 5), paragraph, Attrs::new())
		.expect("split");
	let split = transaction.commit();
	assert_eq!(doc.plain_text(), "hello\n world");
	assert_eq!(
		split.map(Point::new(block, 8), Bias::After).point,
		Point::new(start.block, 3)
	);
	assert_eq!(
		split.map(Point::new(block, 5), Bias::Before).point,
		Point::new(block, 5)
	);
	assert_eq!(split.map(Point::new(block, 5), Bias::After).point, start);

	let mut transaction = doc.transaction();
	let seam = transaction.join(block, start.block).expect("join");
	let join = transaction.commit();
	assert_eq!(doc.plain_text(), "hello world");
	assert_eq!(
		join.map(Point::new(start.block, 3), Bias::After).point,
		Point::new(block, 8)
	);
	assert_eq!(seam, Point::new(block, 5));
}

#[test]
fn join_requires_adjacent_siblings() {
	let mut doc = support::paragraphs(3, "x");
	let ids = doc.children(None).to_vec();

	let result = doc.transaction().join(ids[0], ids[2]);

	assert_eq!(
		result,
		Err(StepError::NotAdjacent {
			block: ids[0],
			next: ids[2]
		})
	);
}

#[test]
fn removed_blocks_map_points_to_the_nearest_survivor() {
	let mut doc = support::paragraphs(3, "abc");
	let ids = doc.children(None).to_vec();

	let mut transaction = doc.transaction();
	transaction
		.remove_blocks(None, 1, 1)
		.expect("remove middle");
	let commit = transaction.commit();

	let mapped = commit.map(Point::new(ids[1], 2), Bias::After);
	assert!(mapped.deleted);
	assert_eq!(mapped.point, Point::new(ids[0], 3));
	assert_eq!(
		commit.map_selection(Selection::caret(Point::new(ids[2], 1))),
		Selection::caret(Point::new(ids[2], 1))
	);
}

#[test]
fn removing_the_first_block_falls_forward() {
	let mut doc = support::paragraphs(2, "abc");
	let ids = doc.children(None).to_vec();

	let mut transaction = doc.transaction();
	transaction.remove_blocks(None, 0, 1).expect("remove first");
	let commit = transaction.commit();

	assert_eq!(
		commit.map(Point::new(ids[0], 1), Bias::After).point,
		Point::new(ids[1], 0)
	);
}

#[test]
fn schema_violations_are_rejected_without_changes() {
	let mut doc = support::paragraphs(2, "abc");
	let before = doc.to_json();
	let item = support::kind(&doc, "list_item");
	let list = support::kind(&doc, "bullet_list");
	let divider = support::kind(&doc, "divider");
	let ids = doc.children(None).to_vec();

	let at_root = doc.transaction().insert_blocks(
		None,
		0,
		vec![BlockTree::container(BlockId::new(7, 0), item, Vec::new())],
	);
	let wrong_content = doc.transaction().set_block(ids[0], list, Attrs::new());
	let duplicate =
		doc.transaction()
			.insert_blocks(None, 0, vec![BlockTree::leaf(ids[0], divider)]);

	assert!(matches!(
		at_root,
		Err(StepError::NotAllowed { parent: None, .. })
	));
	assert_eq!(wrong_content, Err(StepError::ContentMismatch(ids[0])));
	assert_eq!(duplicate, Err(StepError::DuplicateId(ids[0])));
	assert_eq!(doc.to_json(), before);
}

#[test]
fn blocks_cannot_move_inside_themselves() {
	let mut doc = support::paragraphs(1, "x");
	let quote = support::kind(&doc, "blockquote");
	let inner = BlockTree::container(BlockId::new(5, 1), quote, Vec::new());
	let outer = BlockTree::container(BlockId::new(5, 0), quote, vec![inner]);
	doc.transaction()
		.insert_blocks(None, 1, vec![outer])
		.expect("insert quotes");

	let result = doc
		.transaction()
		.move_blocks((None, 1, 1), (Some(BlockId::new(5, 1)), 0));

	assert_eq!(result, Err(StepError::MoveIntoSelf(BlockId::new(5, 0))));
}

#[test]
fn apply_steps_rolls_back_partial_batches() {
	let mut doc = support::paragraphs(1, "abc");
	let block = first(&doc);
	let before = doc.to_json();

	let result = doc.apply_steps([
		Step::RemoveInline { block, range: 0..1 },
		Step::RemoveInline {
			block,
			range: 0..99,
		},
	]);

	assert!(matches!(result, Err(StepError::Inline(_))));
	assert_eq!(doc.to_json(), before);
}

#[test]
fn rollback_restores_the_document() {
	let mut doc = support::paragraphs(2, "abc");
	let before = doc.to_json();
	let paragraph = support::kind(&doc, "paragraph");
	let block = first(&doc);

	let mut transaction = doc.transaction();
	transaction
		.split(Point::new(block, 1), paragraph, Attrs::new())
		.expect("split");
	transaction.remove_blocks(None, 2, 1).expect("remove");
	transaction.rollback().expect("rollback");

	assert_eq!(doc.to_json(), before);
}

#[test]
fn json_round_trips_and_preserves_ids() {
	let mut doc = support::paragraphs(1, "hello");
	let block = first(&doc);
	let mention = Embed {
		kind: doc.schema().embed_type("mention").expect("mention"),
		attrs: Attrs::new().with("id", "ifiok"),
	};
	let link = Mark::with_attrs(
		doc.schema().mark_type("link").expect("link"),
		Attrs::new().with("href", "https://canvist.dev"),
	);
	let mut transaction = doc.transaction();
	transaction
		.step(Step::InsertInline {
			block,
			offset: 5,
			content: InlineText::embed(mention, MarkSet::empty()),
		})
		.expect("embed");
	transaction.add_mark(block, 0..5, link).expect("link");
	let _ = transaction.commit();

	let json = doc.to_json();
	let parsed = Document::from_json(doc.schema().clone(), 3, &json).expect("parses");

	assert_eq!(parsed.to_json(), json);
	assert_eq!(parsed.first_text_block(), Some(block));
}

#[test]
fn imports_tiptap_json_without_ids() {
	let schema = support::schema();
	let tiptap = json!({
		"type": "doc",
		"content": [
			{ "type": "heading", "attrs": { "level": 2, "id": "8b1e0a3c-uuid" }, "content": [{ "type": "text", "text": "Title" }] },
			{ "type": "bullet_list", "content": [
				{ "type": "list_item", "content": [
					{ "type": "paragraph", "content": [
						{ "type": "text", "text": "bold", "marks": [{ "type": "bold" }] },
						{ "type": "text", "text": " and " },
						{ "type": "mention", "attrs": { "id": "ada" } }
					] }
				] }
			] },
			{ "type": "paragraph" }
		]
	});

	let doc = Document::from_json(schema.clone(), 4, &tiptap).expect("imports");
	support::assert_invariants(&doc);

	assert_eq!(doc.plain_text(), "Title\nbold and \u{FFFC}\n");
	assert_eq!(doc.block_count(), 5);
	let heading = doc.block(doc.children(None)[0]).expect("heading");
	assert_eq!(
		heading
			.attrs()
			.get("level")
			.and_then(canvist_model::Value::as_int),
		Some(2)
	);
}

#[test]
fn rejects_unknown_types_in_json() {
	let schema = support::schema();
	let bad = json!({ "type": "doc", "content": [{ "type": "table" }] });

	let error = Document::from_json(schema, 1, &bad).expect_err("table is not in the schema");

	assert_eq!(error.to_string(), "unknown block type `table`");
}
