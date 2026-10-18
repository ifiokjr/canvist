//! What happens to edits a peer makes in the tail of a block that another
//! peer splits at the same time.
//!
//! Yjs has no split, so `Split` deletes the tail from the block's `Y.Text`
//! and writes a copy into the new block's `Y.Text`. Concurrent edits to the
//! tail target the deleted original:
//!
//! - an insert survives, but lands at the end of the first block, cut off
//!   from the text around it;
//! - a deletion is lost: the copy still holds the text;
//! - formatting is lost: the copy keeps the old formatting.
//!
//! Edits before the split point are unaffected. Run with `--nocapture` to
//! print the measurements.

mod support;

use canvist_collab::Encoding;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Step;
use support::Peer;

const TEXT: &str = "abcdef";
const SPLIT: u32 = 3;

/// The concurrent edit the second peer makes.
#[derive(Clone, Copy, Debug)]
enum Edit {
	Insert(u32),
	Delete(u32),
	Bold(u32),
}

/// Split `TEXT` at `SPLIT` on one peer while the other makes `edit`, sync,
/// and return the resulting block texts plus whether any text is bold.
fn race(edit: Edit) -> (Vec<String>, bool) {
	let schema = support::schema();
	let paragraph = schema.default_text_block();
	let mut doc = Document::empty(schema.clone(), 1);
	let block = BlockId::new(1, 0);
	doc.transaction()
		.insert_blocks(
			None,
			0,
			vec![BlockTree::text(
				block,
				paragraph,
				InlineText::plain(TEXT, MarkSet::empty()),
			)],
		)
		.expect("paragraph");
	let mut alice = Peer::seed(doc, 1);
	let mut bob = Peer::join(schema.clone(), 2, &alice.binding.encode_state(Encoding::V1));

	let new_block = alice.doc.allocate_id().expect("id");
	alice
		.step(Step::Split {
			block,
			offset: SPLIT,
			new_block,
			kind: paragraph,
			attrs: Attrs::new(),
		})
		.expect("split");
	let bold = Mark::new(schema.mark_type("bold").expect("bold"));

	let step = match edit {
		Edit::Insert(at) => {
			Step::InsertInline {
				block,
				offset: at,
				content: InlineText::plain("X", MarkSet::empty()),
			}
		}
		Edit::Delete(at) => {
			Step::RemoveInline {
				block,
				range: at..at + 1,
			}
		}
		Edit::Bold(at) => {
			Step::AddMark {
				block,
				range: at..at + 1,
				mark: bold,
			}
		}
	};

	bob.step(step).expect("concurrent edit");

	for update in alice.binding.take_updates() {
		bob.receive(&update);
	}

	for update in bob.binding.take_updates() {
		alice.receive(&update);
	}

	assert_eq!(alice.json(), bob.json(), "peers converge after {edit:?}");
	let texts = alice
		.doc
		.text_blocks()
		.filter_map(|block| block.inline())
		.map(|text| text.as_str().to_owned())
		.collect();
	let bolded = alice
		.doc
		.text_blocks()
		.filter_map(|block| block.inline())
		.any(|text| text.runs().any(|run| !run.marks.is_empty()));
	(texts, bolded)
}

#[test]
fn inserts_into_the_tail_move_to_the_split_point() {
	println!("split {TEXT:?} at {SPLIT}, concurrent insert of X:");

	for at in 0..=6 {
		let (texts, _) = race(Edit::Insert(at));
		println!("  insert at {at}: {texts:?}");
		let expected: [String; 2] = if at <= SPLIT {
			let (head, tail) = TEXT.split_at(SPLIT as usize);
			let mut head = head.to_owned();
			head.insert(at as usize, 'X');
			[head, tail.to_owned()]
		} else {
			[
				format!("{}X", &TEXT[..SPLIT as usize]),
				TEXT[SPLIT as usize..].to_owned(),
			]
		};

		assert_eq!(texts, expected, "insert at {at}");
	}
}

#[test]
fn deletions_in_the_tail_are_lost() {
	println!("split {TEXT:?} at {SPLIT}, concurrent deletion of one character:");

	for at in 0..6 {
		let (texts, _) = race(Edit::Delete(at));
		println!("  delete at {at}: {texts:?}");
		let survives = at < SPLIT;
		let joined = texts.concat();
		assert_eq!(joined.len() == TEXT.len() - 1, survives, "delete at {at}");
	}
}

#[test]
fn formatting_in_the_tail_is_lost() {
	println!("split {TEXT:?} at {SPLIT}, concurrent bold of one character:");

	for at in 0..6 {
		let (_, bolded) = race(Edit::Bold(at));
		println!("  bold at {at}: kept = {bolded}");
		assert_eq!(bolded, at < SPLIT, "bold at {at}");
	}
}
