//! Renders the footage for canvist's launch video.
//!
//! Every frame comes from the real engine: StarterKit editors take scripted
//! keystrokes, `canvist_layout` lays them out with `parley` and the bundled
//! Atkinson Hyperlegible, and `to_svg` paints each block. Collaboration runs
//! three `canvist_collab` peers over Yjs. Keystroke timings are measured
//! while recording, so build in release mode:
//!
//! ```sh
//! cargo run --release -p canvist_footage -- video/public/footage
//! ```
//!
//! The output is one JSON reel per scene and page profile, plus the
//! StarterKit's typed API metadata for the "headless and typed" scene.

mod collab;
mod reel;
mod session;
mod theme;

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use canvist_layout::DocumentLayout;
use canvist_layout::LayoutOptions;
use canvist_layout::Theme;
use canvist_model::Document;
use serde_json::Value as Json;
use serde_json::json;

use crate::reel::Overlay;
use crate::reel::Reel;
use crate::session::Session;

/// Any failure while recording.
pub type Error = Box<dyn std::error::Error>;

/// A page shape: how wide the document is and how large its text is.
#[derive(Clone, Copy, Debug)]
struct Profile {
	name: &'static str,
	width: f32,
	rem: f32,
}

/// Landscape cuts use the wide page; portrait and square cuts the narrow.
const PROFILES: [Profile; 2] = [
	Profile {
		name: "wide",
		width: 1_120.0,
		rem: 30.0,
	},
	Profile {
		name: "narrow",
		width: 920.0,
		rem: 32.0,
	},
];

/// Platform viewports in CSS pixels or points, with the text size each
/// platform's own editor would use.
const PLATFORMS: [(&str, f32, f32); 7] = [
	("web", 760.0, 17.0),
	("desktop", 980.0, 16.0),
	("ios", 390.0, 17.0),
	("android", 412.0, 16.0),
	("flutter", 600.0, 16.0),
	("react-native", 390.0, 17.0),
	("lynx", 375.0, 16.0),
];

const TYPING: &str = "# Every pixel, painted⏎canvist is a rich-text editor whose Rust core lays out and paints every glyph itself, on every platform.";

const FORMATTING: &str = "## Blocks snap into place⏎Markdown as you type: **bold**, *italic*, `code`.⏎- a dash starts a list⏎Enter keeps it going⏎⏎> a chevron makes a quote⏎⏎[[ ] brackets make a task⏎⏎```rust caret.paint();⏎⏎⏎";

fn main() -> Result<(), Error> {
	let out = std::env::args_os()
		.nth(1)
		.map_or_else(|| PathBuf::from("video/public/footage"), PathBuf::from);
	let started = Instant::now();

	for profile in PROFILES {
		let dir = out.join(profile.name);
		std::fs::create_dir_all(&dir)?;
		let theme = theme::wet_caret(profile.rem);

		write(&dir, "typing", &typing(&theme, profile)?)?;
		let (formatting, mut session) = formatting(&theme, profile)?;
		write(&dir, "format", &formatting)?;
		write(&dir, "drag", &drag(&mut session)?)?;
		write(&dir, "collab", &collab::record(theme.clone(), profile.width)?)?;

		if profile.name == "wide" {
			write(&out, "platforms", &platforms(session.doc())?)?;
		}
	}

	let editor = session::editor()?;
	write(&out, "metadata", &editor.metadata_json())?;
	write(
		&out,
		"manifest",
		&json!({
			"target": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
			"profile": if cfg!(debug_assertions) { "debug" } else { "release" },
			"font": theme::FAMILY,
			"profiles": PROFILES.iter().map(|profile| json!({
				"name": profile.name, "width": profile.width, "rem": profile.rem,
			})).collect::<Vec<_>>(),
		}),
	)?;

	eprintln!("footage written to {} in {:.1?}", out.display(), started.elapsed());
	Ok(())
}

fn write(dir: &Path, name: &str, value: &Json) -> Result<(), Error> {
	std::fs::write(dir.join(format!("{name}.json")), serde_json::to_vec(value)?)?;
	Ok(())
}

/// A heading and a paragraph, typed one key at a time.
fn typing(theme: &Arc<Theme>, profile: Profile) -> Result<Json, Error> {
	let mut session = Session::new(theme.clone(), profile.width)?;
	session.still(Overlay::default());
	session.record(TYPING)?;

	Ok(std::mem::replace(&mut session.reel, Reel::new(profile.width))
		.into_json(json!({ "scene": "typing", "rem": profile.rem })))
}

/// Markdown shortcuts turning into marks and blocks. Returns the session so
/// the drag scene continues in the same document.
fn formatting(theme: &Arc<Theme>, profile: Profile) -> Result<(Json, Session), Error> {
	let mut session = Session::new(theme.clone(), profile.width)?;
	session.still(Overlay::default());
	session.record(FORMATTING)?;
	let reel = std::mem::replace(&mut session.reel, Reel::new(profile.width));

	Ok((reel.into_json(json!({ "scene": "format", "rem": profile.rem })), session))
}

/// Drag the code block from the bottom of the document to just below the
/// heading, dropping it into each slot it passes on the way.
fn drag(session: &mut Session) -> Result<Json, Error> {
	let roots = session.doc().children(None).len();
	let code = (0..roots)
		.rfind(|&index| root_type(session.doc(), index) == Some("codeBlock"))
		.ok_or("the formatting script ends with a code block")?;
	let dragged = session.root(code).ok_or("the code block exists")?.to_string();
	session.still(Overlay {
		extra: Some(json!({ "dragging": dragged, "slot": code })),
		..Overlay::default()
	});

	for slot in (1..code).rev() {
		session.move_root(slot + 1, slot, json!({ "dragging": dragged, "slot": slot }))?;
	}

	let reel = std::mem::replace(&mut session.reel, Reel::new(0.0));
	Ok(reel.into_json(json!({ "scene": "drag", "dragging": dragged, "from": code, "to": 1 })))
}

/// The same document laid out at every platform's width, and a sweep of
/// widths between them so the video can show it reflowing.
fn platforms(doc: &Document) -> Result<Json, Error> {
	let mut devices = Vec::new();

	for (name, width, rem) in PLATFORMS {
		let theme = theme::wet_caret(rem);
		let mut reel = Reel::new(width);
		capture_at(doc, &theme, width, &mut reel)?;
		devices.push(reel.into_json(json!({ "platform": name, "rem": rem })));
	}

	let theme = theme::wet_caret(18.0);
	let mut sweep = Reel::new(0.0);
	let widths: Vec<f32> = (0..=48).map(|step| 1_040.0 - step as f32 * 14.0).collect();

	for &width in &widths {
		capture_at(doc, &theme, width, &mut sweep)?;
	}

	Ok(json!({
		"devices": devices,
		"sweep": sweep.into_json(json!({ "widths": widths, "rem": 18.0 })),
	}))
}

fn capture_at(doc: &Document, theme: &Arc<Theme>, width: f32, reel: &mut Reel) -> Result<(), Error> {
	let mut measurer = session::measurer();
	let mut layout = DocumentLayout::new(doc, theme.clone(), LayoutOptions::new(width));
	let start = Instant::now();
	layout.layout_all(doc, &mut measurer)?;
	let micros = start.elapsed().as_secs_f64() * 1e6;
	reel.capture(
		doc,
		&layout,
		None,
		Overlay {
			micros: Some(micros),
			extra: Some(json!({ "width": width })),
			..Overlay::default()
		},
	);
	Ok(())
}

fn root_type(doc: &Document, index: usize) -> Option<&str> {
	let block = doc.block(*doc.children(None).get(index)?)?;
	Some(&doc.schema().block(block.kind()).name)
}
