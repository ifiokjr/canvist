//! Peers, a random editing driver, and a lossy network for collaboration
//! tests.

#![allow(dead_code)]

pub mod deno;
pub mod ops;

use std::sync::Arc;

use canvist_collab::Binding;
use canvist_collab::Encoding;
use canvist_collab::Track;
use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::BlockTree;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::EmbedSpec;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::MarkSpec;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Step;
use canvist_model::Transaction;

/// A StarterKit-like schema with two list types, so concurrent retyping can
/// produce children a container no longer allows.
pub fn schema() -> Arc<Schema> {
	let schema = Schema::builder()
		.block(BlockSpec::text("paragraph"))
		.block(BlockSpec::text("heading").attrs(Attrs::new().with("level", 1)))
		.block(BlockSpec::container("blockquote", &["block"]))
		.block(BlockSpec::container("bullet_list", &["list_item"]))
		.block(BlockSpec::container("ordered_list", &["list_item"]))
		.block(BlockSpec::container("list_item", &["paragraph", "bullet_list"]).groups(&[]))
		.block(BlockSpec::leaf("divider"))
		.mark(MarkSpec::new("bold"))
		.mark(MarkSpec::new("italic"))
		.mark(MarkSpec::new("code").excludes(&["bold", "italic"]))
		.mark(MarkSpec::new("link").inclusive(false))
		.embed(EmbedSpec::new("mention"))
		.build();
	Arc::new(schema.expect("the test schema is valid"))
}

/// One collaborator: a document and its binding.
pub struct Peer {
	pub doc: Document,
	pub binding: Binding,
}

impl Peer {
	/// The first peer, seeding the shared document with `doc`.
	pub fn seed(doc: Document, client: u32) -> Self {
		let mut binding =
			Binding::with_client_id(doc.schema().clone(), u64::from(client)).expect("binding");
		binding
			.seed(&doc)
			.expect("an empty shared document accepts a seed");
		// Joining peers read the seed from the full state instead.
		binding.take_updates();
		Self { doc, binding }
	}

	/// A peer joining from `state`, a full v1 update.
	pub fn join(schema: Arc<Schema>, client: u32, state: &[u8]) -> Self {
		let mut binding =
			Binding::with_client_id(schema.clone(), u64::from(client)).expect("binding");
		let mut doc = Document::empty(schema, client);
		binding
			.apply_update(&mut doc, state, Encoding::V1)
			.expect("a full state applies");
		Self { doc, binding }
	}

	/// Commit a transaction built by `edit` and mirror it, returning whether
	/// the model accepted it.
	pub fn edit(&mut self, edit: impl FnOnce(&mut Transaction<'_>) -> bool) -> Option<Commit> {
		let mut transaction = self.doc.transaction();

		if !edit(&mut transaction) || transaction.is_empty() {
			transaction.rollback().expect("rollback");

			return None;
		}

		let commit = transaction.commit();
		let follow_up = self
			.binding
			.apply_local(&mut self.doc, &commit, Track::Step)
			.expect("local commits translate");
		assert!(
			follow_up.is_empty()
				|| commit
					.steps
					.iter()
					.any(|step| matches!(step, Step::SetBlock { .. })),
			"only retyping reveals hidden blocks"
		);
		Some(commit)
	}

	/// Mirror a commit already applied to the document.
	pub fn record(&mut self, commit: &Commit) {
		self.binding
			.apply_local(&mut self.doc, commit, Track::Step)
			.expect("local commits translate");
	}

	/// Apply one step locally, if the model accepts it.
	pub fn step(&mut self, step: Step) -> Option<Commit> {
		self.edit(|transaction| transaction.step(step).is_ok())
	}

	/// Integrate a remote update.
	pub fn receive(&mut self, update: &[u8]) -> Commit {
		self.binding
			.apply_update(&mut self.doc, update, Encoding::V1)
			.expect("remote updates translate")
	}

	/// The model as JSON.
	pub fn json(&self) -> serde_json::Value {
		self.doc.to_json()
	}

	/// The model rebuilt from scratch out of the shared document.
	pub fn rebuilt(&self) -> serde_json::Value {
		let state = self.binding.encode_state(Encoding::V1);
		let schema = self.doc.schema().clone();
		let mut fresh = Binding::with_client_id(schema.clone(), 999_999).expect("binding");
		let mut doc = Document::empty(schema, 999_999);
		fresh
			.apply_update(&mut doc, &state, Encoding::V1)
			.expect("state applies");
		let rebuilt = fresh.document(999_999).expect("document rebuilds");
		assert_eq!(
			rebuilt.to_json(),
			doc.to_json(),
			"incremental and full reads agree"
		);
		rebuilt.to_json()
	}
}

/// A document with paragraphs, a heading, and a nested list.
pub fn starter(schema: &Arc<Schema>, client: u32) -> Document {
	let mut doc = Document::empty(schema.clone(), client);
	let kind = |name| schema.block_type(name).expect("type exists");
	let bold = Mark::new(schema.mark_type("bold").expect("bold"));
	let mut ids = (0..).map(|seq| BlockId::new(client, seq));
	let mut next = || ids.next().expect("ids");
	let text = |text: &str| InlineText::plain(text, MarkSet::empty());
	let bold_text = InlineText::plain("bold é😀", MarkSet::from_marks([bold]));
	let item = |id, paragraph, content| {
		BlockTree::container(
			id,
			kind("list_item"),
			vec![BlockTree::text(paragraph, kind("paragraph"), content)],
		)
	};

	let blocks = vec![
		BlockTree::text(next(), kind("heading"), text("Title"))
			.with_attrs(Attrs::new().with("level", 1)),
		BlockTree::text(next(), kind("paragraph"), bold_text),
		BlockTree::container(
			next(),
			kind("bullet_list"),
			vec![
				item(next(), next(), text("one")),
				item(next(), next(), text("two")),
			],
		),
		BlockTree::container(
			next(),
			kind("blockquote"),
			vec![BlockTree::text(next(), kind("paragraph"), text("quoted"))],
		),
		BlockTree::leaf(next(), kind("divider")),
		BlockTree::text(next(), kind("paragraph"), text("end")),
	];
	doc.transaction()
		.insert_blocks(None, 0, blocks)
		.expect("starter content is valid");
	doc
}

/// The text of every text block, in order.
pub fn texts(doc: &Document) -> Vec<String> {
	doc.text_blocks()
		.filter_map(Block::inline)
		.map(|text| text.as_str().to_owned())
		.collect()
}

/// The Rust side of the JavaScript compatibility tests, made concurrently
/// with the scripted JavaScript edits in `tests/js/canvist.ts`: type in the
/// title, bold the first list item, and split the last paragraph.
pub fn concurrent_edits(doc: &mut Document) -> Vec<Commit> {
	let schema = doc.schema().clone();
	let title = doc.children(None)[0];
	let list = doc.children(None)[2];
	let one = doc.children(Some(doc.children(Some(list))[0]))[0];
	let end = doc.children(None)[5];
	let bold = Mark::new(schema.mark_type("bold").expect("bold"));
	let paragraph = schema.default_text_block();
	let mut commits = Vec::new();

	let mut transaction = doc.transaction();
	transaction
		.insert_text(Point::new(title, 5), " 2026", MarkSet::empty())
		.expect("type in the title");
	commits.push(transaction.commit());

	let mut transaction = doc.transaction();
	transaction.add_mark(one, 0..3, bold).expect("bold one");
	commits.push(transaction.commit());

	let mut transaction = doc.transaction();
	transaction
		.split(Point::new(end, 1), paragraph, Attrs::new())
		.expect("split the last paragraph");
	commits.push(transaction.commit());
	commits
}

/// The text blocks after `concurrent_edits` and the JavaScript edits merge.
/// The mention JavaScript appended to `end` lands in `e`, because Rust split
/// `end` concurrently and inserts into a split-off tail move to the split
/// point.
pub const MERGED: [&str; 8] = [
	"Title 2026",
	"bold é 🎉",
	"one",
	"two",
	"from js",
	"quoted",
	"e\u{FFFC}",
	"nd",
];

/// Messages in flight between peers, delivered in any order and sometimes
/// twice.
#[derive(Default)]
pub struct Network {
	queue: Vec<(usize, Vec<u8>)>,
}

impl Network {
	/// Queue every update `from` produced for all other peers.
	pub fn broadcast(&mut self, from: usize, peers: &mut [Peer]) {
		for update in peers[from].binding.take_updates() {
			for to in (0..peers.len()).filter(|&to| to != from) {
				self.queue.push((to, update.clone()));
			}
		}
	}

	/// Deliver the message picked by `choice`, keeping a copy if `duplicate`.
	pub fn deliver(&mut self, choice: u32, duplicate: bool, peers: &mut [Peer]) {
		if self.queue.is_empty() {
			return;
		}

		let index = choice as usize % self.queue.len();
		let (to, update) = if duplicate {
			self.queue[index].clone()
		} else {
			self.queue.swap_remove(index)
		};

		peers[to].receive(&update);
		self.broadcast(to, peers);
	}

	/// Deliver everything until the network is quiet.
	pub fn drain(&mut self, peers: &mut [Peer]) {
		while !self.queue.is_empty() {
			self.deliver(0, false, peers);
		}
	}
}

/// Panic unless every structural invariant of `doc` holds and every block
/// is allowed where it is.
pub fn assert_invariants(doc: &Document) {
	let mut seen = 0;
	check_children(doc, None, &mut seen);
	assert_eq!(seen, doc.block_count(), "every block is reachable once");
}

fn check_children(doc: &Document, parent: Option<BlockId>, seen: &mut usize) {
	let parent_kind = parent.and_then(|id| doc.block(id)).map(Block::kind);

	for &child in doc.children(parent) {
		let block = doc.block(child).expect("child ids resolve");
		*seen += 1;
		assert_eq!(block.parent(), parent, "parent pointer of {child}");
		assert!(
			doc.schema().allows_child(parent_kind, block.kind()),
			"schema allows {child} here"
		);
		check_children(doc, Some(child), seen);
	}
}
