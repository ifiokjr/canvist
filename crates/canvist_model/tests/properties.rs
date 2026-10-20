//! Property tests: random edit sequences never break invariants, failed
//! steps never change the document, and undoing every commit restores the
//! original.

mod support;

use std::ops::Range;

use canvist_model::Attrs;
use canvist_model::Bias;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::Embed;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Step;
use canvist_model::TableMap;
use proptest::prelude::*;

/// An edit described by seeds, resolved against the current document so
/// that most generated edits are valid.
#[derive(Clone, Debug)]
enum Edit {
	Type {
		block: usize,
		at: usize,
		text: String,
	},
	Delete {
		block: usize,
		from: usize,
		to: usize,
	},
	Format {
		block: usize,
		from: usize,
		to: usize,
		mark: usize,
		add: bool,
	},
	Embed {
		block: usize,
		at: usize,
	},
	Split {
		block: usize,
		at: usize,
	},
	Join {
		block: usize,
	},
	Heading {
		block: usize,
	},
	Wrap {
		block: usize,
	},
	Unwrap {
		block: usize,
	},
	Remove {
		block: usize,
	},
}

fn edit() -> impl Strategy<Value = Edit> {
	let seed = || 0..64usize;
	prop_oneof![
		4 => (seed(), seed(), "[a-zé🎨 \\n]{0,6}").prop_map(|(block, at, text)| Edit::Type { block, at, text }),
		2 => (seed(), seed(), seed()).prop_map(|(block, from, to)| Edit::Delete { block, from, to }),
		2 => (seed(), seed(), seed(), 0..4usize, any::<bool>())
			.prop_map(|(block, from, to, mark, add)| Edit::Format { block, from, to, mark, add }),
		1 => (seed(), seed()).prop_map(|(block, at)| Edit::Embed { block, at }),
		2 => (seed(), seed()).prop_map(|(block, at)| Edit::Split { block, at }),
		1 => seed().prop_map(|block| Edit::Join { block }),
		1 => seed().prop_map(|block| Edit::Heading { block }),
		1 => seed().prop_map(|block| Edit::Wrap { block }),
		1 => seed().prop_map(|block| Edit::Unwrap { block }),
		1 => seed().prop_map(|block| Edit::Remove { block }),
	]
}

fn text_block(doc: &Document, seed: usize) -> BlockId {
	let blocks: Vec<BlockId> = doc.text_blocks().map(Block::id).collect();
	blocks[seed % blocks.len()]
}

/// A character boundary chosen by `seed`.
fn boundary(doc: &Document, block: BlockId, seed: usize) -> u32 {
	let text = doc.inline(block).map_or("", InlineText::as_str);
	let boundaries: Vec<usize> = text
		.char_indices()
		.map(|(index, _)| index)
		.chain([text.len()])
		.collect();
	boundaries[seed % boundaries.len()] as u32
}

fn resolve(doc: &mut Document, edit: &Edit) -> Step {
	match *edit {
		Edit::Type {
			block,
			at,
			ref text,
		} => {
			let (block, offset) = caret(doc, block, at);
			let content = InlineText::plain(text, MarkSet::empty());
			Step::InsertInline {
				block,
				offset,
				content,
			}
		}
		Edit::Delete { block, from, to } => {
			let (block, range) = span(doc, block, from, to);
			Step::RemoveInline { block, range }
		}
		Edit::Format {
			block,
			from,
			to,
			mark,
			add,
		} => format(doc, span(doc, block, from, to), mark, add),
		Edit::Embed { block, at } => embed(doc, caret(doc, block, at)),
		Edit::Split { block, at } => split(doc, caret(doc, block, at)),
		Edit::Join { block } => {
			let block = text_block(doc, block);
			let next = doc.next_text_block(block).unwrap_or(block);
			Step::Join { block, next }
		}
		Edit::Heading { block } => heading(doc, text_block(doc, block)),
		Edit::Wrap { block } => wrap(doc, text_block(doc, block)),
		Edit::Unwrap { block } => unwrap(doc, text_block(doc, block)),
		Edit::Remove { block } => {
			let (parent, index) = location(doc, text_block(doc, block));
			Step::RemoveBlocks {
				parent,
				index,
				count: 1,
			}
		}
	}
}

/// A text block and a character boundary inside it, chosen by seeds.
fn caret(doc: &Document, block: usize, at: usize) -> (BlockId, u32) {
	let block = text_block(doc, block);
	(block, boundary(doc, block, at))
}

/// A text block and an ordered range inside it, chosen by seeds.
fn span(doc: &Document, block: usize, from: usize, to: usize) -> (BlockId, Range<u32>) {
	let block = text_block(doc, block);
	let (from, to) = (boundary(doc, block, from), boundary(doc, block, to));
	(block, from.min(to)..from.max(to))
}

/// A block's parent and index within it.
fn location(doc: &Document, block: BlockId) -> (Option<BlockId>, usize) {
	let parent = doc.block(block).and_then(Block::parent);
	(parent, doc.index_in_parent(block).unwrap_or(0))
}

fn format(doc: &Document, (block, range): (BlockId, Range<u32>), mark: usize, add: bool) -> Step {
	let kind = doc.schema().marks()[mark].id;

	if add {
		return Step::AddMark {
			block,
			range,
			mark: Mark::new(kind),
		};
	}

	Step::RemoveMark { block, range, kind }
}

fn embed(doc: &Document, (block, offset): (BlockId, u32)) -> Step {
	let kind = doc.schema().embed_type("mention").expect("mention");
	let embed = Embed {
		kind,
		attrs: Attrs::new(),
	};
	let content = InlineText::embed(embed, MarkSet::empty());
	Step::InsertInline {
		block,
		offset,
		content,
	}
}

fn split(doc: &mut Document, (block, offset): (BlockId, u32)) -> Step {
	let new_block = doc.allocate_id().expect("ids available");
	let kind = doc.block(block).expect("exists").kind();
	Step::Split {
		block,
		offset,
		new_block,
		kind,
		attrs: Attrs::new(),
	}
}

fn heading(doc: &Document, block: BlockId) -> Step {
	let kind = doc.schema().block_type("heading").expect("heading");
	Step::SetBlock {
		block,
		kind,
		attrs: Attrs::new().with("level", 2),
	}
}

/// Insert a blockquote holding an empty paragraph before `block`, so later
/// edits move blocks in and out of containers.
fn wrap(doc: &mut Document, block: BlockId) -> Step {
	let quote = doc.schema().block_type("blockquote").expect("blockquote");
	let paragraph = doc.schema().default_text_block();
	let (parent, index) = location(doc, block);
	let inner = BlockTree::text(
		doc.allocate_id().expect("ids"),
		paragraph,
		InlineText::new(),
	);
	let tree = BlockTree::container(doc.allocate_id().expect("ids"), quote, vec![inner]);
	Step::InsertBlocks {
		parent,
		index,
		blocks: vec![tree],
	}
}

/// Move `block` out of its container to the front of the grandparent.
fn unwrap(doc: &Document, block: BlockId) -> Step {
	let (parent, index) = location(doc, block);
	let to_parent = parent.and_then(|id| doc.block(id)).and_then(Block::parent);
	Step::MoveBlocks {
		parent,
		index,
		count: 1,
		to_parent,
		to_index: 0,
	}
}

/// Apply every edit, checking invariants after each, then undo every commit
/// in reverse. Failed edits must leave the document byte-for-byte unchanged,
/// which is why each attempt snapshots the JSON first.
fn edit_then_undo(edits: &[Edit]) -> Result<(), TestCaseError> {
	let mut doc = support::paragraphs(3, "hello wörld");
	let original = doc.to_json();
	let mut commits = Vec::new();

	for edit in edits {
		if doc.first_text_block().is_none() {
			break;
		}

		let before = doc.to_json();
		let step = resolve(&mut doc, edit);
		let applied = doc.apply_steps([step]);
		support::assert_invariants(&doc);

		let Ok(commit) = applied else {
			prop_assert_eq!(&doc.to_json(), &before, "failed steps change nothing");
			continue;
		};
		commits.push(commit);
	}

	for commit in commits.iter().rev() {
		doc.apply_steps(commit.undo_steps())
			.expect("undo always applies");
		support::assert_invariants(&doc);
	}

	prop_assert_eq!(doc.to_json(), original);
	Ok(())
}

/// Map a caret through every successful edit and require it to stay a valid
/// position, unless its block and every other text block were removed.
fn caret_survives(edits: &[Edit], seed: usize) -> Result<(), TestCaseError> {
	let mut doc = support::paragraphs(2, "abc def");
	let (block, offset) = caret(&doc, seed, seed);
	let mut point = Point::new(block, offset);

	for edit in edits {
		if doc.first_text_block().is_none() {
			return Ok(());
		}

		let step = resolve(&mut doc, edit);

		if let Ok(commit) = doc.apply_steps([step]) {
			point = commit.map(point, Bias::After).point;
		}

		let Some(text) = doc.inline(point.block) else {
			prop_assert!(
				!doc.contains(point.block),
				"only removed blocks lose carets"
			);

			return Ok(());
		};
		prop_assert!(
			text.check_offset(point.offset).is_ok(),
			"caret {:?} valid in {:?}",
			point,
			text.as_str()
		);
	}

	Ok(())
}

/// Everything a savepoint must restore: content, version, every block's
/// revision, and the next id the document would allocate.
fn fingerprint(doc: &Document) -> (serde_json::Value, u64, Vec<(BlockId, u32)>, Option<BlockId>) {
	let mut revs: Vec<(BlockId, u32)> = doc.iter().map(|block| (block.id(), block.rev())).collect();
	revs.sort_unstable();
	let next = doc.clone().allocate_id().ok();
	(doc.to_json(), doc.version(), revs, next)
}

fn apply_all(doc: &mut Document, edits: &[Edit]) {
	for edit in edits {
		if doc.first_text_block().is_none() {
			return;
		}

		let step = resolve(doc, edit);
		let _ = doc.apply_steps([step]);
	}
}

/// Edit inside a savepoint (and a nested one, kept or reverted), then roll
/// back: the document must match the fingerprint from before, exactly.
fn savepoint_restores(
	prefix: &[Edit],
	outer: &[Edit],
	inner: &[Edit],
	keep_inner: bool,
) -> Result<(), TestCaseError> {
	let mut doc = support::paragraphs(3, "hello wörld");
	apply_all(&mut doc, prefix);
	let before = fingerprint(&doc);
	let savepoint = doc.savepoint();
	apply_all(&mut doc, outer);
	let middle = fingerprint(&doc);
	let nested = doc.savepoint();
	apply_all(&mut doc, inner);

	if keep_inner {
		doc.release(nested);
	} else {
		doc.rollback_to(nested).expect("inner rollback");
		prop_assert_eq!(&fingerprint(&doc), &middle, "inner rollback is exact");
	}

	doc.rollback_to(savepoint).expect("outer rollback");
	support::assert_invariants(&doc);
	prop_assert_eq!(fingerprint(&doc), before, "outer rollback is exact");
	Ok(())
}

proptest! {
	#![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

	#[test]
	fn savepoints_roll_back_without_a_trace(
		prefix in prop::collection::vec(edit(), 0..10),
		outer in prop::collection::vec(edit(), 0..20),
		inner in prop::collection::vec(edit(), 0..20),
		keep_inner in any::<bool>(),
	) {
		savepoint_restores(&prefix, &outer, &inner, keep_inner)?;
	}

	#[test]
	fn random_edits_preserve_invariants_and_undo_exactly(edits in prop::collection::vec(edit(), 1..60)) {
		edit_then_undo(&edits)?;
	}

	#[test]
	fn caret_mapping_stays_inside_the_document(edits in prop::collection::vec(edit(), 1..40), seed in 0..64usize) {
		caret_survives(&edits, seed)?;
	}

	#[test]
	fn table_maps_cover_each_slot_once(
		rows in prop::collection::vec(prop::collection::vec((1..4usize, 1..4usize), 0..5), 1..6),
	) {
		table_map_is_consistent(&rows)?;
	}
}

/// Every cell of a table map covers exactly the slots its spans say, no
/// slot belongs to two cells, and each cell is found again by its row and
/// index.
fn table_map_is_consistent(rows: &[Vec<(usize, usize)>]) -> Result<(), TestCaseError> {
	let map = TableMap::new(rows.iter().map(|row| row.iter().copied()));
	let mut covered = 0;

	for (index, cell) in map.cells().iter().enumerate() {
		prop_assert!(cell.rows().end <= map.height());
		prop_assert!(cell.columns().end <= map.width());
		prop_assert_eq!(map.find(cell.row, cell.index), Some(index));

		for row in cell.rows() {
			for column in cell.columns() {
				prop_assert_eq!(map.cell_at(row, column), Some(index));
				covered += 1;
			}
		}
	}

	let filled = (0..map.height())
		.flat_map(|row| (0..map.width()).map(move |column| (row, column)))
		.filter(|&(row, column)| map.cell_at(row, column).is_some())
		.count();
	prop_assert_eq!(filled, covered, "no slot is covered twice");
	prop_assert_eq!(map.cells().len(), rows.iter().map(Vec::len).sum::<usize>());
	Ok(())
}

/// Simulates a fast typist: thousands of single-character insertions,
/// backspaces, and Enters with no allocation blow-up or panic.
#[test]
fn sustained_typing_burst_stays_consistent() {
	let mut doc = support::paragraphs(1, "");
	let paragraph = doc.schema().default_text_block();
	let mut caret = Point::new(doc.first_text_block().expect("block"), 0);
	let script = "The quick brown fox jumps over the lazy dog. ";

	for (index, character) in script.chars().cycle().take(20_000).enumerate() {
		let mut transaction = doc.transaction();

		caret = match index % 97 {
			0 => {
				transaction
					.split(caret, paragraph, Attrs::new())
					.expect("enter")
			}

			13 if caret.offset > 0 => {
				let previous = caret.offset - 1;
				transaction
					.delete_text(caret.block, previous..caret.offset)
					.expect("backspace");
				Point::new(caret.block, previous)
			}

			_ => {
				transaction
					.insert_text(caret, character.encode_utf8(&mut [0; 4]), MarkSet::empty())
					.expect("type")
			}
		};

		let _ = transaction.commit();
	}

	support::assert_invariants(&doc);
	assert!(doc.block_count() > 200);
	assert!(doc.plain_text().len() > 18_000);
}
