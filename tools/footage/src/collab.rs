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

/// While writing apart, updates are delivered every this many keystrokes,
/// so the peers' edits overlap.
const BATCH: usize = 3;

struct Peer {
	user: User,
	doc: Document,
	binding: Binding,
	presence: Presence,
	caret: Point,
}

impl Peer {
	fn typed(&mut self, text: &str) -> Result<canvist_model::Commit, Error> {
		let mut transaction = self.doc.transaction();
		transaction.insert_text(self.caret, text, MarkSet::empty())?;
		let commit = transaction.commit();
		self.binding.apply_local(&mut self.doc, &commit, Track::Step)?;
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
}

/// What the collaboration scene measured.
#[derive(Debug, Default)]
struct Stats {
	updates: usize,
	bytes: usize,
	remote_micros: Vec<f64>,
}

/// Record the scene into a reel `width` pixels wide.
pub fn record(theme: Arc<Theme>, width: f32) -> Result<Json, Error> {
	let mut setup = Session::new(theme.clone(), width)?;
	setup.prepare("## One page, three carets⏎⏎⏎⏎")?;
	let seed = setup.doc().clone();
	let schema = seed.schema().clone();
	let lines: Vec<BlockId> = seed.children(None)[1..5].to_vec();

	let mut first = Binding::with_client_id(schema.clone(), 1)?;
	first.seed(&seed)?;
	let state = first.encode_state(Encoding::V1);
	let mut first = Some((seed.clone(), first));
	let mut peers = Vec::new();

	for (index, (name, color, _)) in PEERS.iter().enumerate() {
		let client = index as u32 + 1;
		let (doc, binding) = if let Some(seeded) = first.take() {
			seeded
		} else {
			let mut binding = Binding::with_client_id(schema.clone(), u64::from(client))?;
			let mut doc = Document::empty(schema.clone(), client);
			binding.apply_update(&mut doc, &state, Encoding::V1)?;
			(doc, binding)
		};
		let presence = Presence::new(&binding);
		peers.push(Peer {
			user: User {
				name: (*name).to_owned(),
				color: (*color).to_owned(),
			},
			doc,
			binding,
			presence,
			caret: Point::new(lines[index], 0),
		});
	}

	let mut view = View::new(&peers[0].doc, theme, width)?;
	let mut stats = Stats::default();
	let mut frames = 0;
	let apart: Vec<Vec<char>> = PEERS.iter().map(|(.., line)| line.chars().collect()).collect();
	let longest = apart.iter().map(Vec::len).max().unwrap_or(0);

	sync(&mut peers, &mut view, &mut stats)?;
	view.capture(&mut peers, "apart")?;

	for step in 0..longest {
		for (index, (peer, line)) in peers.iter_mut().zip(&apart).enumerate() {
			if let Some(&next) = line.get(step) {
				let commit = peer.typed(&next.to_string())?;

				if index == 0 {
					view.layout.apply(&peer.doc, &commit);
				}
			}
		}

		frames += 1;

		if frames % BATCH == 0 {
			sync(&mut peers, &mut view, &mut stats)?;
		}

		view.capture(&mut peers, "apart")?;
	}

	sync(&mut peers, &mut view, &mut stats)?;
	view.capture(&mut peers, "apart")?;

	let meeting = lines[3];

	for peer in &mut peers {
		peer.caret = Point::new(meeting, 0);
	}

	view.capture(&mut peers, "converge")?;

	for (turn, word) in TOGETHER.iter().enumerate() {
		let index = turn % peers.len();
		let end = text_len(&peers[index].doc, meeting);
		peers[index].caret = Point::new(meeting, end);

		for next in word.chars() {
			let commit = peers[index].typed(&next.to_string())?;

			if index == 0 {
				view.layout.apply(&peers[0].doc, &commit);
			}

			sync(&mut peers, &mut view, &mut stats)?;
			view.capture(&mut peers, "together")?;
		}
	}

	let converged = peers
		.windows(2)
		.all(|pair| pair[0].doc.to_json() == pair[1].doc.to_json());
	let median = median(&mut stats.remote_micros);

	Ok(view.reel.into_json(json!({
		"scene": "collab",
		"peers": PEERS.iter().map(|(name, color, _)| json!({ "name": name, "color": color })).collect::<Vec<_>>(),
		"converged": converged,
		"updates": stats.updates,
		"bytesPerUpdate": stats.bytes.checked_div(stats.updates).unwrap_or(0),
		"remoteMedianMicros": median,
	})))
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
		for peer in peers.iter_mut() {
			peer.publish();
		}

		let states: Vec<Vec<u8>> = peers
			.iter()
			.map(|peer| peer.presence.encode())
			.collect::<Result<_, _>>()?;

		for (index, peer) in peers.iter_mut().enumerate() {
			for (from, state) in states.iter().enumerate() {
				if from != index {
					peer.presence.apply(state)?;
				}
			}
		}

		let viewer = &peers[0];
		let mut carets = vec![(
			viewer.user.name.clone(),
			viewer.user.color.clone(),
			Selection::caret(viewer.caret),
		)];

		for collaborator in viewer.presence.collaborators(&viewer.binding, &viewer.doc) {
			if let (Some(user), Some(selection)) = (collaborator.user, collaborator.selection) {
				carets.push((user.name, user.color, selection));
			}
		}

		self.layout
			.ensure(&viewer.doc, &mut self.measurer, 0.0, 4_000.0)?;
		self.reel.capture(
			&viewer.doc,
			&self.layout,
			None,
			Overlay {
				peers: carets,
				extra: Some(json!({ "phase": phase })),
				..Overlay::default()
			},
		);
		Ok(())
	}
}

/// Deliver every pending update to every other peer, timing how long the
/// first peer takes to apply each remote update.
fn sync(peers: &mut [Peer], view: &mut View, stats: &mut Stats) -> Result<(), Error> {
	let outgoing: Vec<Vec<Vec<u8>>> = peers
		.iter_mut()
		.map(|peer| peer.binding.take_updates())
		.collect();

	for (from, updates) in outgoing.iter().enumerate() {
		for update in updates {
			stats.updates += 1;
			stats.bytes += update.len();

			for (index, peer) in peers.iter_mut().enumerate() {
				if index == from {
					continue;
				}

				let start = Instant::now();
				let commit = peer.binding.apply_update(&mut peer.doc, update, Encoding::V1)?;
				peer.caret = commit.map(peer.caret, Bias::Before).point;

				if index == 0 {
					view.layout.apply(&peer.doc, &commit);
					stats.remote_micros.push(start.elapsed().as_secs_f64() * 1e6);
				}
			}
		}
	}

	Ok(())
}

fn text_len(doc: &Document, block: BlockId) -> u32 {
	doc.block(block)
		.and_then(|block| match block.content() {
			canvist_model::Content::Inline(text) => Some(text.len()),
			_ => None,
		})
		.unwrap_or(0)
}

fn median(values: &mut [f64]) -> Option<f64> {
	if values.is_empty() {
		return None;
	}

	values.sort_by(f64::total_cmp);
	Some((values[values.len() / 2] * 100.0).round() / 100.0)
}
