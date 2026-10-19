//! Three peers editing one document over Yjs, seen from the first peer.
//!
//! Each peer has its own document and [`Binding`]; their updates and
//! awareness states travel between them as bytes, delivered in batches so
//! edits really are concurrent. The first peer's layout paints every frame,
//! and every caret on it is a Yjs relative position resolved by
//! [`Presence`].

use std::sync::Arc;
use std::time::Instant;

use canvist_collab::Binding;
use canvist_collab::Encoding;
use canvist_collab::Presence;
use canvist_collab::Track;
use canvist_collab::User;
use canvist_layout::DocumentLayout;
use canvist_layout::LayoutOptions;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Theme;
use canvist_model::Bias;
use canvist_model::BlockId;
use canvist_model::Commit;
use canvist_model::Content;
use canvist_model::Document;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Selection;
use serde_json::Value as Json;
use serde_json::json;

use crate::Error;
use crate::reel::Overlay;
use crate::reel::Reel;
use crate::session::Session;
use crate::session::measurer;

/// The collaborators: name, caret colour, and what they write on their
/// own line before meeting on the last one.
const PEERS: [(&str, &str, &str); 3] = [
	("Ada", "#3B2CF5", "Ada tightens the intro."),
	("Kofi", "#E5484D", "Kofi rewrites the middle."),
	("Mei", "#12A594", "Mei drafts the ending."),
];

/// The words the three write together, in turn, on the shared last line.
const TOGETHER: [&str; 6] = ["We", " met", " in", " one", " sentence", "."];

/// The seed document: a heading, then a line for each peer and one more
/// they share, which must be the last.
const SEED: &str = "## One page, three carets⏎⏎⏎⏎";

/// While writing apart, updates are delivered every this many keystrokes,
/// so the peers' edits overlap.
const BATCH: usize = 3;

/// How far below the top the viewer's layout keeps the document measured.
const VIEWPORT: f32 = 4_000.0;

/// Record the scene into a reel `width` pixels wide.
pub fn record(theme: Arc<Theme>, width: f32) -> Result<Json, Error> {
	let mut room = Room::open(theme, width)?;
	room.write_apart()?;
	room.write_together()?;
	Ok(room.into_json())
}

struct Peer {
	user: User,
	doc: Document,
	binding: Binding,
	presence: Presence,
	caret: Point,
}

impl Peer {
	fn new(user: (&str, &str), (doc, binding): (Document, Binding), line: BlockId) -> Self {
		let presence = Presence::new(&binding);

		Self {
			user: User {
				name: user.0.to_owned(),
				color: user.1.to_owned(),
			},
			doc,
			binding,
			presence,
			caret: Point::new(line, 0),
		}
	}

	fn typed(&mut self, text: &str) -> Result<Commit, Error> {
		let mut transaction = self.doc.transaction();
		transaction.insert_text(self.caret, text, MarkSet::empty())?;
		let commit = transaction.commit();
		self.binding
			.apply_local(&mut self.doc, &commit, Track::Step)?;
		self.caret = commit.map(self.caret, Bias::After).point;
		Ok(commit)
	}

	fn publish(&mut self) {
		self.presence.set_local(
			(&self.binding, &self.doc),
			&self.user,
			Some(Selection::caret(self.caret)),
		);
	}

	/// Apply a remote update, keeping this peer's caret on its characters.
	fn receive(&mut self, update: &[u8]) -> Result<Commit, Error> {
		let commit = self
			.binding
			.apply_update(&mut self.doc, update, Encoding::V1)?;
		self.caret = commit.map(self.caret, Bias::Before).point;
		Ok(commit)
	}
}

/// What the collaboration scene measured.
#[derive(Debug, Default)]
struct Stats {
	updates: usize,
	bytes: usize,
	remote_micros: Vec<f64>,
}

/// The three peers, the first peer's view of them, and what was measured.
struct Room {
	peers: Vec<Peer>,
	lines: Vec<BlockId>,
	view: View,
	stats: Stats,
}

impl Room {
	/// Seed the document on the first peer and join the others from its
	/// encoded state, as a server would hand it to them.
	fn open(theme: Arc<Theme>, width: f32) -> Result<Self, Error> {
		let mut setup = Session::new(theme.clone(), width)?;
		setup.prepare(SEED)?;
		let seed = setup.doc().clone();
		let schema = seed.schema().clone();
		let lines = seed.children(None)[1..].to_vec();

		let mut first = Binding::with_client_id(schema.clone(), 1)?;
		first.seed(&seed)?;
		let shared = first.encode_state(Encoding::V1);
		let mut peers = vec![Peer::new((PEERS[0].0, PEERS[0].1), (seed, first), lines[0])];

		for (client, (name, color, _)) in (2..).zip(PEERS.iter().skip(1)) {
			let mut binding = Binding::with_client_id(schema.clone(), u64::from(client))?;
			let mut doc = Document::empty(schema.clone(), client);
			binding.apply_update(&mut doc, &shared, Encoding::V1)?;
			peers.push(Peer::new((name, color), (doc, binding), lines[peers.len()]));
		}

		let view = View::new(&peers[0].doc, theme, width)?;
		let mut room = Self {
			peers,
			lines,
			view,
			stats: Stats::default(),
		};
		room.sync()?;
		room.capture("apart")?;
		Ok(room)
	}

	/// Every peer types its own line at once, a character per frame, with
	/// updates delivered in batches.
	fn write_apart(&mut self) -> Result<(), Error> {
		let lines: Vec<Vec<char>> = PEERS
			.iter()
			.map(|(.., line)| line.chars().collect())
			.collect();
		let longest = lines.iter().map(Vec::len).max().unwrap_or(0);

		for step in 0..longest {
			for (index, line) in lines.iter().enumerate() {
				if let Some(next) = line.get(step) {
					self.type_on(index, *next)?;
				}
			}

			if (step + 1) % BATCH == 0 {
				self.sync()?;
			}

			self.capture("apart")?;
		}

		self.sync()?;
		self.capture("apart")
	}

	/// All three carets move to the shared line, then take turns writing
	/// one sentence, each word delivered as it is typed.
	fn write_together(&mut self) -> Result<(), Error> {
		let meeting = self.lines[PEERS.len()];

		for peer in &mut self.peers {
			peer.caret = Point::new(meeting, 0);
		}

		self.capture("converge")?;

		for (turn, word) in TOGETHER.iter().enumerate() {
			let index = turn % self.peers.len();
			let end = text_len(&self.peers[index].doc, meeting);
			self.peers[index].caret = Point::new(meeting, end);

			for next in word.chars() {
				self.type_on(index, next)?;
				self.sync()?;
				self.capture("together")?;
			}
		}

		Ok(())
	}

	fn type_on(&mut self, index: usize, next: char) -> Result<(), Error> {
		let peer = &mut self.peers[index];
		let commit = peer.typed(&next.to_string())?;

		if index == 0 {
			self.view.layout.apply(&peer.doc, &commit);
		}

		Ok(())
	}

	/// Deliver every pending update to every other peer, timing how long
	/// the first peer takes to apply each one and relay out.
	fn sync(&mut self) -> Result<(), Error> {
		let outgoing: Vec<Vec<Vec<u8>>> = self
			.peers
			.iter_mut()
			.map(|peer| peer.binding.take_updates())
			.collect();

		for (from, updates) in outgoing.iter().enumerate() {
			for update in updates {
				self.deliver(from, update)?;
			}
		}

		Ok(())
	}

	fn deliver(&mut self, from: usize, update: &[u8]) -> Result<(), Error> {
		self.stats.updates += 1;
		self.stats.bytes += update.len();

		for (index, peer) in self.peers.iter_mut().enumerate() {
			if index == from {
				continue;
			}

			let start = Instant::now();
			let commit = peer.receive(update)?;

			if index == 0 {
				self.view.layout.apply(&peer.doc, &commit);
				self.stats
					.remote_micros
					.push(start.elapsed().as_secs_f64() * 1e6);
			}
		}

		Ok(())
	}

	fn capture(&mut self, phase: &str) -> Result<(), Error> {
		self.view.capture(&mut self.peers, phase)
	}

	fn into_json(mut self) -> Json {
		let converged = self
			.peers
			.windows(2)
			.all(|pair| pair[0].doc.to_json() == pair[1].doc.to_json());
		let peers: Vec<Json> = PEERS
			.iter()
			.map(|(name, color, _)| json!({ "name": name, "color": color }))
			.collect();
		let meta = json!({
			"scene": "collab",
			"peers": peers,
			"converged": converged,
			"updates": self.stats.updates,
			"bytesPerUpdate": self.stats.bytes.checked_div(self.stats.updates).unwrap_or(0),
			"remoteMedianMicros": median(&mut self.stats.remote_micros),
		});

		self.view.reel.into_json(&meta)
	}
}

/// The first peer's layout and the reel it paints.
struct View {
	layout: DocumentLayout,
	measurer: ParleyMeasurer,
	reel: Reel,
}

impl View {
	fn new(doc: &Document, theme: Arc<Theme>, width: f32) -> Result<Self, Error> {
		let mut measurer = measurer();
		let mut layout = DocumentLayout::new(doc, theme, LayoutOptions::new(width));
		layout.layout_all(doc, &mut measurer)?;

		Ok(Self {
			layout,
			measurer,
			reel: Reel::new(width),
		})
	}

	/// Exchange presence and capture the first peer's view, with every
	/// caret resolved from Yjs relative positions.
	fn capture(&mut self, peers: &mut [Peer], phase: &str) -> Result<(), Error> {
		exchange_presence(peers)?;

		let viewer = &peers[0];
		let mut carets = vec![(
			viewer.user.name.clone(),
			viewer.user.color.clone(),
			Selection::caret(viewer.caret),
		)];
		let collaborators = viewer.presence.collaborators(&viewer.binding, &viewer.doc);

		for collaborator in collaborators {
			if let (Some(user), Some(selection)) = (collaborator.user, collaborator.selection) {
				carets.push((user.name, user.color, selection));
			}
		}

		self.layout
			.ensure(&viewer.doc, &mut self.measurer, 0.0, VIEWPORT)?;
		let overlay = Overlay {
			peers: carets,
			extra: Some(json!({ "phase": phase })),
			..Overlay::default()
		};
		self.reel.capture(&viewer.doc, &self.layout, None, &overlay);
		Ok(())
	}
}

/// Publish every peer's caret and deliver each awareness state to the
/// others.
fn exchange_presence(peers: &mut [Peer]) -> Result<(), Error> {
	for peer in peers.iter_mut() {
		peer.publish();
	}

	let states: Vec<Vec<u8>> = peers
		.iter()
		.map(|peer| peer.presence.encode())
		.collect::<Result<_, _>>()?;

	for (index, peer) in peers.iter_mut().enumerate() {
		let others = states.iter().enumerate().filter(|(from, _)| *from != index);

		for (_, state) in others {
			peer.presence.apply(state)?;
		}
	}

	Ok(())
}

fn text_len(doc: &Document, block: BlockId) -> u32 {
	let content = doc.block(block).map(canvist_model::Block::content);

	match content {
		Some(Content::Inline(text)) => text.len(),
		_ => 0,
	}
}

/// The median, to hundredths.
fn median(values: &mut [f64]) -> Option<f64> {
	if values.is_empty() {
		return None;
	}

	values.sort_by(f64::total_cmp);
	Some((values[values.len() / 2] * 100.0).round() / 100.0)
}
