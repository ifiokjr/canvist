//! The loopback agent: the agent protocol against an editor in the
//! daemon's own process.
//!
//! It is the reference implementation of `wire.rs` — the Swift and Kotlin
//! agents do what this does — and the way the protocol itself is tested
//! with no device at all: harness to daemon to loopback agent to a real
//! `canvist_headless` editor and back.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use serde_json::Value as Json;
use serde_json::json;

use crate::daemon::Context;
use crate::httpio;
use crate::wire::Instruction;
use crate::wire::Load;
use crate::wire::Op;
use crate::wire::Read;
use crate::wire::Reply;

/// Start the loopback agent loop for this daemon. It ends when the daemon
/// says `done` or stops answering.
pub(crate) fn spawn(context: &Arc<Context>, generation: u64) {
	let context = Arc::clone(context);

	tokio::spawn(async move {
		if let Err(error) = agent(&context, generation).await {
			eprintln!("[testkit] loopback agent ended: {error}");
		}
	});
}

/// One editor the agent is driving, with what it needs for the next op.
struct Editor {
	engine: canvist_headless::Engine,
	doc: canvist_headless::Doc,
	/// The marked (preedit) text, which `UnmarkText` commits as it is.
	marked: Option<String>,
	/// The typing clock, kept a heartbeat apart so typing groups into undo
	/// steps the way it does on a real host.
	clock: Duration,
}

/// The agent loop: ask for work, do it, answer.
async fn agent(context: &Arc<Context>, _generation: u64) -> Result<(), String> {
	let port = context.port;
	let mut editors: std::collections::HashMap<u64, Editor> = std::collections::HashMap::new();

	loop {
		let answer = httpio::request(
			port,
			"GET",
			"/next?agent=loopback",
			None,
			Duration::from_secs(70),
		)
		.await
		.map_err(|error| format!("could not reach the daemon (next): {error}"))?;

		let instruction: Instruction =
			serde_json::from_value(answer).map_err(|error| error.to_string())?;

		let reply = match instruction {
			Instruction::Wait => {
				tokio::time::sleep(Duration::from_millis(250)).await;

				continue;
			}
			Instruction::Done => return Ok(()),
			Instruction::Start {
				id,
				session,
				extensions,
				client,
				apple,
				load,
			} => {
				match start(&extensions, client, apple, &load) {
					Ok(built) => {
						editors.insert(session, built);

						Reply::ok(id, Json::Null)
					}
					Err(error) => Reply::failed(id, error),
				}
			}
			Instruction::Op { id, session, op } => {
				match editors.get_mut(&session) {
					Some(current) => run(current, op, id),
					None => Reply::failed(id, "no editor for this session"),
				}
			}
			Instruction::Read { id, session, what } => {
				match editors.get(&session) {
					Some(current) => read(current, what, id),
					None => Reply::failed(id, "no editor for this session"),
				}
			}
			Instruction::Close { id, session } => {
				editors.remove(&session);

				Reply::ok(id, Json::Null)
			}
		};

		let body = serde_json::to_value(&reply).map_err(|error| error.to_string())?;
		httpio::request(port, "POST", "/reply", Some(&body), Duration::from_secs(30))
			.await
			.map_err(|error| format!("could not post a reply ({body}): {error}"))?;
	}
}

/// Build the session's editor and load its document.
fn start(extensions: &[String], client: u32, apple: bool, load: &Load) -> Result<Editor, String> {
	let names: Vec<&str> = extensions.iter().map(String::as_str).collect();
	let platform = if apple {
		canvist_headless::canvist_core::Platform::Apple
	} else {
		canvist_headless::canvist_core::Platform::Other
	};

	let engine = canvist_headless::Engine::named(&names)
		.map_err(|error| error.to_string())?
		.with_client(client)
		.with_platform(platform);

	// `yjs` joins a shared document as a peer; every other format loads
	// into a fresh one.
	let doc = if load.format == "yjs" {
		#[cfg(feature = "collab")]
		{
			let state = decode(&load.content)?;

			engine
				.from_yjs(&state, u64::from(client.max(1)))
				.map_err(|error| error.to_string())?
		}
		#[cfg(not(feature = "collab"))]
		{
			return Err("joining a shared document needs the collab Cargo feature".to_owned());
		}
	} else {
		let format: canvist_headless::Format = load
			.format
			.parse()
			.map_err(|error: canvist_headless::HeadlessError| error.to_string())?;

		engine
			.parse(format, &load.content)
			.map_err(|error| error.to_string())?
	};

	Ok(Editor {
		engine,
		doc,
		marked: None,
		clock: Duration::ZERO,
	})
}

/// Run one operation. Every arm maps one wire op to one editor call, the
/// way the device agents map them to FFI calls.
fn run(editor: &mut Editor, op: Op, id: u64) -> Reply {
	let result: Result<Json, String> = match op {
		Op::Load { format, content } => {
			return match parse(&format).and_then(|format| {
				editor
					.engine
					.parse(format, &content)
					.map(|doc| {
						editor.doc = doc;
					})
					.map_err(|error| error.to_string())
			}) {
				Ok(()) => Reply::ok(id, Json::Null),
				Err(error) => Reply::failed(id, error),
			};
		}
		Op::TypeText { text } => {
			type_like_a_keyboard(editor, &text)
				.map(Json::Bool)
				.map_err(|error| stringed(&error))
		}
		Op::InsertText { text } => {
			editor
				.doc
				.insert_text(&text)
				.map(Json::Bool)
				.map_err(|error| stringed(&error))
		}
		Op::Press {
			key,
			shift,
			ctrl,
			alt,
			meta,
		} => {
			editor
				.doc
				.press(&binding(&key, shift, ctrl, alt, meta))
				.map(Json::Bool)
				.map_err(|error| stringed(&error))
		}
		Op::Select { anchor, head } => {
			let head = head.unwrap_or_else(|| anchor.clone());

			return editor
				.doc
				.select_range(&anchor.into(), &head.into())
				.map_or_else(
					|error| Reply::failed(id, error.to_string()),
					|()| Reply::ok(id, Json::Null),
				);
		}
		Op::SelectAll => {
			editor
				.doc
				.select_all()
				.map(|()| Json::Bool(true))
				.map_err(|error| stringed(&error))
		}
		Op::DeleteSelection => {
			editor
				.doc
				.delete_selection()
				.map(Json::Bool)
				.map_err(|error| stringed(&error))
		}
		Op::Paste { format, content } => {
			parse(&format).and_then(|format| {
				editor
					.doc
					.paste(format, &content)
					.map(Json::Bool)
					.map_err(|error| stringed(&error))
			})
		}
		Op::Run { command, args } => {
			editor
				.doc
				.run_named(&command, &args)
				.map(Json::Bool)
				.map_err(|error| stringed(&error))
		}
		Op::Can { command, args } => {
			editor
				.doc
				.can_named(&command, &args)
				.map(Json::Bool)
				.map_err(|error| stringed(&error))
		}
		Op::Query { query, args } => {
			editor
				.doc
				.query(&query, &args)
				.map_err(|error| stringed(&error))
		}
		Op::Undo => {
			editor
				.doc
				.undo()
				.map(Json::Bool)
				.map_err(|error| stringed(&error))
		}
		Op::Redo => {
			editor
				.doc
				.redo()
				.map(Json::Bool)
				.map_err(|error| stringed(&error))
		}
		Op::SetMarkedText { text, .. } => {
			return match mark(editor, &text) {
				Ok(()) => Reply::ok(id, Json::Null),
				Err(error) => Reply::failed(id, error.to_string()),
			};
		}
		Op::UnmarkText => {
			let marked = editor.marked.take().unwrap_or_default();

			return match mark(editor, &marked).and_then(|()| commit(editor, &marked)) {
				Ok(()) => Reply::ok(id, Json::Null),
				Err(error) => Reply::failed(id, error.to_string()),
			};
		}
		Op::Share { client } => return op_share(editor, id, client),
		Op::ApplyUpdate { update } => return op_apply_update(editor, id, &update),
		Op::TakeUpdates => return op_take_updates(editor, id),
		Op::EncodeState => return op_encode_state(editor, id),
	};

	match result {
		Ok(value) => Reply::ok(id, value),
		Err(error) => Reply::failed(id, error),
	}
}

/// `Share`, which only a `collab` build can serve.
#[cfg(feature = "collab")]
fn op_share(editor: &mut Editor, id: u64, client: u64) -> Reply {
	editor.doc.share(client).map_or_else(
		|error| Reply::failed(id, error.to_string()),
		|()| Reply::ok(id, Json::Null),
	)
}

/// [`op_share`] without the feature.
#[cfg(not(feature = "collab"))]
fn op_share(editor: &mut Editor, id: u64, client: u64) -> Reply {
	let _ = (editor, client);

	Reply::failed(id, "share needs the collab Cargo feature")
}

/// `ApplyUpdate`, which only a `collab` build can serve.
#[cfg(feature = "collab")]
fn op_apply_update(editor: &mut Editor, id: u64, update: &str) -> Reply {
	decode(update)
		.and_then(|update| {
			editor
				.doc
				.apply_update(&update)
				.map_err(|error| stringed(&error))
		})
		.map_or_else(
			|error| Reply::failed(id, error),
			|changed| Reply::ok(id, Json::Bool(changed)),
		)
}

/// [`op_apply_update`] without the feature.
#[cfg(not(feature = "collab"))]
fn op_apply_update(editor: &mut Editor, id: u64, update: &str) -> Reply {
	let _ = (editor, update);

	Reply::failed(id, "applyUpdate needs the collab Cargo feature")
}

/// `TakeUpdates`, which only a `collab` build can serve.
#[cfg(feature = "collab")]
fn op_take_updates(editor: &mut Editor, id: u64) -> Reply {
	editor.doc.take_updates().map_or_else(
		|error| Reply::failed(id, error.to_string()),
		|updates| {
			Reply::ok(
				id,
				Json::Array(
					updates
						.iter()
						.map(|update| Json::String(encode(update.as_slice())))
						.collect(),
				),
			)
		},
	)
}

/// [`op_take_updates`] without the feature.
#[cfg(not(feature = "collab"))]
fn op_take_updates(editor: &mut Editor, id: u64) -> Reply {
	let _ = editor;

	Reply::failed(id, "takeUpdates needs the collab Cargo feature")
}

/// `EncodeState`, which only a `collab` build can serve.
#[cfg(feature = "collab")]
fn op_encode_state(editor: &mut Editor, id: u64) -> Reply {
	editor.doc.encode_state().map_or_else(
		|error| Reply::failed(id, error.to_string()),
		|state| Reply::ok(id, Json::String(encode(state.as_slice()))),
	)
}

/// [`op_encode_state`] without the feature.
#[cfg(not(feature = "collab"))]
fn op_encode_state(editor: &mut Editor, id: u64) -> Reply {
	let _ = editor;

	Reply::failed(id, "encodeState needs the collab Cargo feature")
}

/// Answer one read.
fn read(editor: &Editor, what: Read, id: u64) -> Reply {
	let value = match what {
		Read::Text => Json::String(editor.doc.export(canvist_headless::Format::Text)),
		Read::Markdown => Json::String(editor.doc.export(canvist_headless::Format::Markdown)),
		Read::Html => Json::String(editor.doc.export(canvist_headless::Format::Html)),
		Read::Json => Json::String(editor.doc.export(canvist_headless::Format::Json)),
		Read::Selection => {
			match editor.doc.selected_range() {
				Some((anchor, head)) => {
					json!({
						"anchor": crate::Position::from(anchor),
						"head": crate::Position::from(head),
					})
				}
				None => Json::Null,
			}
		}
		Read::CanUndo => Json::Bool(editor.doc.can_undo()),
		Read::CanRedo => Json::Bool(editor.doc.can_redo()),
	};

	Reply::ok(id, value)
}

/// Type `text` one character at a time, a heartbeat apart, so input rules
/// fire and the undo history groups the word — what a real keyboard does.
fn type_like_a_keyboard(
	editor: &mut Editor,
	text: &str,
) -> Result<bool, canvist_headless::HeadlessError> {
	let mut changed = false;
	let mut buffer = [0; 4];

	for character in text.chars() {
		let typed = character.encode_utf8(&mut buffer);
		editor.clock += Duration::from_millis(100);
		let update = editor.doc.edit(
			editor.clock,
			&mut canvist_headless::canvist_core::ParagraphNavigator,
			|document, env| document.insert_text(typed, env),
		)?;

		changed |= update.is_some();
	}

	Ok(changed)
}

/// Set the marked (preedit) text.
fn mark(editor: &mut Editor, text: &str) -> Result<(), canvist_headless::HeadlessError> {
	edit(editor, |document, env| document.set_composition(text, env))?;
	editor.marked = Some(text.to_owned());

	Ok(())
}

/// Commit the marked text as `text`.
fn commit(editor: &mut Editor, text: &str) -> Result<(), canvist_headless::HeadlessError> {
	edit(editor, |document, env| {
		document.commit_composition(text, env)
	})?;
	editor.marked = None;

	Ok(())
}

/// Run one editor call through the document's own clock.
fn edit(
	editor: &mut Editor,
	change: impl FnOnce(
		&mut canvist_headless::canvist_core::Editor,
		&mut canvist_headless::canvist_core::Env<'_>,
	) -> canvist_headless::canvist_core::CommandResult,
) -> Result<(), canvist_headless::HeadlessError> {
	use std::time::Duration;

	use canvist_headless::canvist_core::ParagraphNavigator;

	editor
		.doc
		.edit(Duration::ZERO, &mut ParagraphNavigator, change)
		.map(|_| ())
}

/// A binding string for concrete modifiers, so the headless keymap reads
/// what the device agents' keymaps read.
#[allow(
	clippy::fn_params_excessive_bools,
	reason = "one flag per modifier, the wire op's shape"
)]
fn binding(key: &str, shift: bool, ctrl: bool, alt: bool, meta: bool) -> String {
	let mut binding = String::new();

	for (held, name) in [
		(ctrl, "Ctrl-"),
		(alt, "Alt-"),
		(meta, "Meta-"),
		(shift, "Shift-"),
	] {
		if held {
			binding.push_str(name);
		}
	}

	binding.push_str(key);
	binding
}

/// Parse a format name.
fn parse(format: &str) -> Result<canvist_headless::Format, String> {
	format
		.parse()
		.map_err(|error: canvist_headless::HeadlessError| error.to_string())
}

/// Base64-encode one update.
fn encode(update: &[u8]) -> String {
	base64::engine::general_purpose::STANDARD.encode(update)
}

/// Base64-decode one update.
fn decode(update: &str) -> Result<Vec<u8>, String> {
	base64::engine::general_purpose::STANDARD
		.decode(update)
		.map_err(|error| error.to_string())
}

/// An editor error as a string, for `map_err`.
fn stringed(error: &canvist_headless::HeadlessError) -> String {
	error.to_string()
}
