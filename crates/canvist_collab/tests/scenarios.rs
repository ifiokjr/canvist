//! Specific concurrent situations and what the binding does with them.

mod support;

use canvist_collab::Encoding;
use canvist_collab::Track;
use canvist_collab::keys;
use canvist_collab::yrs;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Step;
use support::Peer;
use yrs::Any;
use yrs::Array;
use yrs::ArrayRef;
use yrs::Map;
use yrs::MapPrelim;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;
use yrs::Text;
use yrs::TextPrelim;
use yrs::TextRef;
use yrs::Transact;
use yrs::types::Attrs as Format;
use yrs::updates::decoder::Decode;

/// Two peers sharing a document of three paragraphs, `one`, `two`, `three`.
fn pair() -> (Peer, Peer) {
	let schema = support::schema();
	let paragraph = schema.default_text_block();
	let mut doc = Document::empty(schema.clone(), 1);
	let blocks = ["one", "two", "three"]
		.into_iter()
		.zip(0..)
		.map(|(text, seq)| {
			BlockTree::text(
				BlockId::new(1, seq),
				paragraph,
				InlineText::plain(text, MarkSet::empty()),
			)
		})
		.collect();
	doc.transaction()
		.insert_blocks(None, 0, blocks)
		.expect("paragraphs");
	let alice = Peer::seed(doc, 1);
	let bob = Peer::join(schema, 2, &alice.binding.encode_state(Encoding::V1));
	(alice, bob)
}

/// Deliver every pending update in both directions until both are quiet.
fn sync(a: &mut Peer, b: &mut Peer) {
	loop {
		let from_a = a.binding.take_updates();
		let from_b = b.binding.take_updates();

		if from_a.is_empty() && from_b.is_empty() {
			return;
		}

		for update in from_a {
			b.receive(&update);
		}

		for update in from_b {
			a.receive(&update);
		}
	}
}

fn texts(peer: &Peer) -> Vec<String> {
	peer.doc
		.text_blocks()
		.filter_map(|block| block.inline())
		.map(|text| text.as_str().to_owned())
		.collect()
}

fn block(peer: &Peer, index: usize) -> BlockId {
	peer.doc.children(None)[index]
}

#[test]
fn concurrent_moves_of_one_block_keep_one_copy() {
	let (mut alice, mut bob) = pair();
	let one = block(&alice, 0);
	let move_one = |to_index| {
		Step::MoveBlocks {
			parent: None,
			index: 0,
			count: 1,
			to_parent: None,
			to_index,
		}
	};

	alice.step(move_one(1)).expect("alice moves one after two");
	bob.step(move_one(2)).expect("bob moves one to the end");
	sync(&mut alice, &mut bob);

	assert_eq!(alice.json(), bob.json());
	assert_eq!(
		alice
			.doc
			.children(None)
			.iter()
			.filter(|&&id| id == one)
			.count(),
		1
	);
	assert_eq!(texts(&alice).len(), 3, "no paragraph is duplicated or lost");
	assert_eq!(alice.rebuilt(), alice.json(), "the losing copy was deleted");
}

#[test]
fn a_peer_receiving_both_copies_at_once_keeps_one() {
	let (mut alice, mut bob) = pair();
	let state = alice.binding.encode_state(Encoding::V1);
	let move_first = |to_index| {
		Step::MoveBlocks {
			parent: None,
			index: 0,
			count: 1,
			to_parent: None,
			to_index,
		}
	};
	alice.step(move_first(1)).expect("alice moves one");
	bob.step(move_first(2)).expect("bob moves one");

	// Both moves reach a raw Yjs document before anyone repairs, so a peer
	// joining from it reads two live copies of `one` in one update.
	let merged = Foreign::join(&state);

	for update in [alice.binding.take_updates(), bob.binding.take_updates()].concat() {
		let update = yrs::Update::decode_v1(&update).expect("decodes");
		merged
			.doc
			.transact_mut()
			.apply_update(update)
			.expect("applies");
	}

	let joined = merged
		.doc
		.transact()
		.encode_state_as_update_v1(&yrs::StateVector::default());
	let mut carol = Peer::join(support::schema(), 3, &joined);

	assert_eq!(texts(&carol).len(), 3, "carol shows one copy");
	assert_eq!(
		carol.binding.take_updates().len(),
		1,
		"and deletes the other"
	);
	sync(&mut alice, &mut bob);
	assert_eq!(
		carol.json(),
		alice.json(),
		"the copy carol keeps is everyone's"
	);
}

#[test]
fn a_remote_move_keeps_the_block_and_its_carets() {
	let (mut alice, mut bob) = pair();
	let one = block(&alice, 0);

	alice
		.step(Step::MoveBlocks {
			parent: None,
			index: 0,
			count: 1,
			to_parent: None,
			to_index: 2,
		})
		.expect("move");
	let updates = alice.binding.take_updates();
	assert_eq!(updates.len(), 1, "one commit, one update");
	let commit = bob.receive(&updates[0]);

	assert!(
		commit
			.steps
			.iter()
			.any(|step| matches!(step, Step::MoveBlocks { .. })),
		"a delete-and-copy arrives as a move: {:?}",
		commit.steps
	);
	let caret = commit.map(Point::new(one, 2), canvist_model::Bias::After);
	assert_eq!(caret.point, Point::new(one, 2));
	assert!(!caret.deleted);
	assert_eq!(texts(&bob), ["two", "three", "one"]);
}

#[test]
fn a_child_its_container_no_longer_allows_is_hidden_until_allowed_again() {
	let (mut alice, mut bob) = pair();
	let schema = alice.doc.schema().clone();
	let quote = BlockId::new(1, 100);
	let blockquote = schema.block_type("blockquote").expect("blockquote");
	alice
		.step(Step::InsertBlocks {
			parent: None,
			index: 3,
			blocks: vec![BlockTree::container(quote, blockquote, Vec::new())],
		})
		.expect("an empty quote");
	sync(&mut alice, &mut bob);

	let bullet_list = schema.block_type("bullet_list").expect("bullet_list");
	alice
		.step(Step::SetBlock {
			block: quote,
			kind: bullet_list,
			attrs: Attrs::new(),
		})
		.expect("an empty quote can become a list");
	let paragraph = BlockTree::text(
		BlockId::new(2, 100),
		schema.default_text_block(),
		InlineText::plain("quoted", MarkSet::empty()),
	);
	bob.step(Step::InsertBlocks {
		parent: Some(quote),
		index: 0,
		blocks: vec![paragraph],
	})
	.expect("a quote holds paragraphs");
	sync(&mut alice, &mut bob);

	assert_eq!(alice.json(), bob.json());
	assert!(
		alice.doc.children(Some(quote)).is_empty(),
		"a list cannot hold a paragraph"
	);
	support::assert_invariants(&bob.doc);

	alice
		.step(Step::SetBlock {
			block: quote,
			kind: blockquote,
			attrs: Attrs::new(),
		})
		.expect("back to a quote");
	sync(&mut alice, &mut bob);

	assert_eq!(alice.json(), bob.json());
	assert_eq!(texts(&bob).last().map(String::as_str), Some("quoted"));
}

/// A raw Yjs peer, writing the shared document directly.
struct Foreign {
	doc: yrs::Doc,
	root: ArrayRef,
}

impl Foreign {
	fn join(state: &[u8]) -> Self {
		let doc = yrs::Doc::with_options(yrs::Options {
			offset_kind: yrs::OffsetKind::Utf16,
			..yrs::Options::with_client_id(yrs::block::ClientID::new(77))
		});
		let root = doc.get_or_insert_array(keys::ROOT);
		let update = yrs::Update::decode_v1(state).expect("state decodes");
		doc.transact_mut()
			.apply_update(update)
			.expect("state applies");
		Self { doc, root }
	}

	/// Run `edit` in one transaction and return its update.
	fn edit(&self, edit: impl FnOnce(&mut yrs::TransactionMut<'_>, &ArrayRef)) -> Vec<u8> {
		let mut txn = self.doc.transact_mut();
		edit(&mut txn, &self.root);
		txn.encode_update_v1()
	}

	fn block(&self, txn: &yrs::TransactionMut<'_>, index: u32) -> MapRef {
		match self.root.get(txn, index) {
			Some(Out::YMap(map)) => map,
			other => panic!("expected a block map, found {other:?}"),
		}
	}

	fn text(&self, txn: &yrs::TransactionMut<'_>, index: u32) -> TextRef {
		match self.block(txn, index).get(txn, keys::TEXT) {
			Some(Out::YText(text)) => text,
			other => panic!("expected a text, found {other:?}"),
		}
	}
}

#[test]
fn blocks_of_unknown_types_are_hidden_kept_and_shown_once_valid() {
	let (mut alice, _) = pair();
	assert_eq!(alice.binding.rejected_blocks(), 0);
	let foreign = Foreign::join(&alice.binding.encode_state(Encoding::V1));
	let update = foreign.edit(|txn, root| {
		let callout: MapRef = root.insert(txn, 1, MapPrelim::default());
		callout.insert(txn, keys::ID, "1z.0");
		callout.insert(txn, keys::TYPE, "callout");
		let text: TextRef = callout.insert(txn, keys::TEXT, TextPrelim::new("note"));
		text.insert(txn, 4, "!");
	});

	alice.receive(&update);
	assert_eq!(
		texts(&alice),
		["one", "two", "three"],
		"the callout is hidden"
	);
	assert_eq!(alice.binding.rejected_blocks(), 1);

	// Local edits still land in the right place around the hidden map.
	let two = block(&alice, 1);
	alice
		.edit(|tx| {
			tx.insert_text(Point::new(two, 3), "!", MarkSet::empty())
				.is_ok()
		})
		.expect("edit");
	alice
		.step(Step::MoveBlocks {
			parent: None,
			index: 2,
			count: 1,
			to_parent: None,
			to_index: 1,
		})
		.expect("move three before two");
	let fix = {
		for update in alice.binding.take_updates() {
			let update = yrs::Update::decode_v1(&update).expect("decodes");
			foreign
				.doc
				.transact_mut()
				.apply_update(update)
				.expect("applies");
		}

		foreign.edit(|txn, _| {
			let callout = foreign.block(txn, 1);
			callout.insert(txn, keys::TYPE, "paragraph");
		})
	};
	alice.receive(&fix);

	assert_eq!(texts(&alice), ["one", "note!", "three", "two!"]);
	assert_eq!(
		alice.binding.rejected_blocks(),
		0,
		"the callout is valid now"
	);
	assert_eq!(alice.rebuilt(), alice.json());
}

#[test]
fn unknown_marks_are_kept_and_unknown_embeds_show_as_replacements() {
	let (mut alice, _) = pair();
	let foreign = Foreign::join(&alice.binding.encode_state(Encoding::V1));
	let update = foreign.edit(|txn, _| {
		let text = foreign.text(txn, 0);
		let comment = Format::from([(std::sync::Arc::from("comment"), Any::from("c1"))]);
		text.format(txn, 0, 3, comment);
		let image = Any::from(std::collections::HashMap::from([(
			"image".to_owned(),
			Any::from("cat.png"),
		)]));
		text.insert_embed(txn, 1, image);
	});

	let commit = alice.receive(&update);
	assert_eq!(texts(&alice)[0], "o\u{FFFD}ne");
	assert!(
		commit
			.steps
			.iter()
			.all(|step| !matches!(step, Step::RestoreMarks { .. }))
	);

	let one = block(&alice, 0);
	alice
		.edit(|tx| {
			tx.insert_text(Point::new(one, 4), "x", MarkSet::empty())
				.is_ok()
		})
		.expect("typing after the replacement character");

	for update in alice.binding.take_updates() {
		let update = yrs::Update::decode_v1(&update).expect("decodes");
		foreign
			.doc
			.transact_mut()
			.apply_update(update)
			.expect("applies");
	}

	let txn = foreign.doc.transact_mut();
	let chunks = foreign.text(&txn, 0).diff(&txn, |_| ());
	let strings: String = chunks
		.iter()
		.filter_map(|chunk| {
			match &chunk.insert {
				Out::Any(Any::String(text)) => Some(text.to_string()),
				_ => None,
			}
		})
		.collect();
	assert_eq!(strings, "oxne");
	assert!(
		chunks
			.iter()
			.any(|chunk| matches!(&chunk.insert, Out::Any(Any::Map(_)))),
		"the image embed survives"
	);
	assert!(
		chunks.iter().any(|chunk| {
			chunk
				.attributes
				.as_ref()
				.is_some_and(|format| format.contains_key("comment"))
		}),
		"the comment survives"
	);
}

#[test]
fn undo_reverts_only_local_changes() {
	let (mut alice, mut bob) = pair();
	let one = block(&alice, 0);
	alice
		.edit(|tx| {
			tx.insert_text(Point::new(one, 3), " by alice", MarkSet::empty())
				.is_ok()
		})
		.expect("alice types");
	sync(&mut alice, &mut bob);
	bob.edit(|tx| {
		tx.insert_text(Point::new(one, 0), "bob: ", MarkSet::empty())
			.is_ok()
	})
	.expect("bob types");
	sync(&mut alice, &mut bob);

	let undone = alice
		.binding
		.undo(&mut alice.doc)
		.expect("undo translates")
		.expect("something to undo");
	assert!(!undone.is_empty());
	sync(&mut alice, &mut bob);
	assert_eq!(texts(&bob)[0], "bob: one");
	assert!(alice.binding.can_redo());

	alice.binding.redo(&mut alice.doc).expect("redo translates");
	sync(&mut alice, &mut bob);
	assert_eq!(texts(&bob)[0], "bob: one by alice");
	assert_eq!(alice.json(), bob.json());
}

#[test]
fn merged_commits_undo_together() {
	let (mut alice, _) = pair();
	let one = block(&alice, 0);

	for (offset, track) in [(3, Track::Step), (4, Track::Merge), (5, Track::Merge)] {
		let mut tx = alice.doc.transaction();
		tx.insert_text(Point::new(one, offset), "!", MarkSet::empty())
			.expect("type");
		let commit = tx.commit();
		alice
			.binding
			.apply_local(&mut alice.doc, &commit, track)
			.expect("local");
	}

	alice.binding.undo(&mut alice.doc).expect("undo");
	assert_eq!(texts(&alice)[0], "one");
	assert!(!alice.binding.can_undo());
}
