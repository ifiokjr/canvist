//! The protocol between the test process and an agent: the device-side
//! script every target's agent implements.
//!
//! The Swift agent (`platforms/apple/Tests/CanvistTests/TestkitAgent.swift`)
//! and the Kotlin agent (`platforms/android/.../TestkitAgent.kt`) implement
//! this over HTTP; the loopback agent in `agent.rs` implements it in this
//! process, which is how the protocol itself is tested without a device.
//!
//! The server owns one instruction stream. An agent asks for the next
//! instruction, executes it against a real editor, and posts the reply:
//!
//! ```text
//! GET  /next?agent=<name>   -> {"id": 7, "kind": "op", "session": 0, "op": {...}}
//!                          |  {"id": 7, "kind": "start" | "read" | "close", ...}
//!                          |  {"kind": "wait"}    // nothing yet; poll again
//!                          |  {"kind": "done"}    // no more work; exit
//! POST /reply               -> {"id": 7, "ok": true, "value": ...}
//! ```
//!
//! A session is one editor: created by `start`, driven by `op`, read by
//! `read`, released by `close`.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as Json;

use crate::Position;

/// What a session starts from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Load {
	/// The format of `content`: `json`, `html`, `markdown`, or `text`.
	pub format: String,
	/// The document, written in that format.
	pub content: String,
}

/// One editor operation, as an agent runs it.
///
/// The names and shapes follow the conformance fixture ops, so every
/// runner agrees on what typing means: `TypeText` goes one character at a
/// time, so input rules fire; `InsertText` replaces the selection whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Op {
	/// Replace the document with `content` read as `format`.
	Load {
		/// The format of `content`.
		format: String,
		/// The document, written in that format.
		content: String,
	},
	/// Type `text` one character at a time, as a keyboard would.
	TypeText {
		/// What to type.
		text: String,
	},
	/// Insert `text` at the selection in one call, without input rules.
	InsertText {
		/// What to insert.
		text: String,
	},
	/// Press a key with concrete modifiers; the testkit resolves `Mod`
	/// per platform before sending.
	Press {
		/// The key, as a DOM `KeyboardEvent.key`: `Enter`, `Backspace`,
		/// `b`.
		key: String,
		/// Shift held.
		shift: bool,
		/// Control held.
		ctrl: bool,
		/// Alt or Option held.
		alt: bool,
		/// Command or Meta held.
		meta: bool,
	},
	/// Select from `anchor` to `head`, or place a caret when `head` is
	/// `None`.
	Select {
		/// Where the selection starts.
		anchor: Position,
		/// Where it ends.
		head: Option<Position>,
	},
	/// Select the whole document.
	SelectAll,
	/// Delete the selection.
	DeleteSelection,
	/// Paste `content` read as `format`, running paste rules.
	Paste {
		/// The format of `content`.
		format: String,
		/// What to paste.
		content: String,
	},
	/// Run the command an extension registered as `command`.
	Run {
		/// The command name, such as `toggleBold`.
		command: String,
		/// Its arguments, or `null`.
		args: Json,
	},
	/// Whether the command would apply. Nothing changes.
	Can {
		/// The command name.
		command: String,
		/// Its arguments, or `null`.
		args: Json,
	},
	/// Answer the query an extension registered as `query`.
	Query {
		/// The query name, such as `isBold`.
		query: String,
		/// Its arguments, or `null`.
		args: Json,
	},
	/// Undo the last change this peer made.
	Undo,
	/// Redo the last undone change.
	Redo,
	/// IME: set the marked (preedit) text, selecting `selected` within it.
	SetMarkedText {
		/// The composing text.
		text: String,
		/// The range within it the user selected, in UTF-16 units.
		selected: (u32, u32),
	},
	/// IME: commit the marked text as typed.
	UnmarkText,
	/// Start sharing this editor as Yjs client `client`.
	Share {
		/// The client id to allocate as.
		client: u64,
	},
	/// Integrate a v1 Yjs update from another editor, base64-encoded.
	/// Returns whether the document changed.
	ApplyUpdate {
		/// The update, base64.
		update: String,
	},
	/// Drain this editor's v1 updates for broadcasting, base64-encoded.
	TakeUpdates,
	/// The whole shared state as one v1 update, base64-encoded.
	EncodeState,
}

/// What a harness read asks an agent for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Read {
	/// The document as plain text.
	Text,
	/// The document as Markdown.
	Markdown,
	/// The document as HTML.
	Html,
	/// The document as ProseMirror JSON, serialized.
	Json,
	/// The selection as `{anchor, head}`, or `null` for a block selection.
	Selection,
	/// Whether there is anything to undo.
	CanUndo,
	/// Whether there is anything to redo.
	CanRedo,
}

impl Read {
	/// The name the wire writes.
	#[must_use]
	pub fn name(self) -> &'static str {
		match self {
			Self::Text => "text",
			Self::Markdown => "markdown",
			Self::Html => "html",
			Self::Json => "json",
			Self::Selection => "selection",
			Self::CanUndo => "canUndo",
			Self::CanRedo => "canRedo",
		}
	}
}

/// One instruction from the server to an agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Instruction {
	/// Create the session's editor and load its document.
	Start {
		/// Identifies the instruction; the reply repeats it.
		id: u64,
		/// The session this editor belongs to.
		session: u64,
		/// The extensions to register, by name.
		extensions: Vec<String>,
		/// The block-id client this editor allocates as.
		client: u32,
		/// Whether `Mod` is Command: true on Apple platforms, false
		/// elsewhere.
		apple: bool,
		/// The document to load.
		load: Load,
	},
	/// Run one operation on the session's editor.
	Op {
		/// Identifies the instruction; the reply repeats it.
		id: u64,
		/// The session to run it on.
		session: u64,
		/// What to run.
		op: Op,
	},
	/// Read state from the session's editor.
	Read {
		/// Identifies the instruction; the reply repeats it.
		id: u64,
		/// The session to read.
		session: u64,
		/// What to read.
		what: Read,
	},
	/// Release the session's editor.
	Close {
		/// Identifies the instruction; the reply repeats it.
		id: u64,
		/// The session to release.
		session: u64,
	},
	/// Nothing to do right now; ask again.
	Wait,
	/// No more work; leave the loop and let the run finish.
	Done,
}

impl Instruction {
	/// The instruction's id, or `None` for `Wait` and `Done`, which need no
	/// reply.
	#[must_use]
	pub fn id(&self) -> Option<u64> {
		match self {
			Self::Start { id, .. }
			| Self::Op { id, .. }
			| Self::Read { id, .. }
			| Self::Close { id, .. } => Some(*id),
			Self::Wait | Self::Done => None,
		}
	}
}

/// An agent's answer to one instruction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reply {
	/// The instruction this answers.
	pub id: u64,
	/// Whether it ran. `false` carries `error`.
	pub ok: bool,
	/// What it produced: the `bool` of an op, the value of a query or a
	/// read, `null` for `start` and `close`.
	pub value: Json,
	/// Why it did not run, when `ok` is `false`.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub error: Option<String>,
}

impl Reply {
	/// A reply that ran, producing `value`.
	#[must_use]
	pub fn ok(id: u64, value: Json) -> Self {
		Self {
			id,
			ok: true,
			value,
			error: None,
		}
	}

	/// A reply that did not run, for `reason`.
	#[must_use]
	pub fn failed(id: u64, reason: impl Into<String>) -> Self {
		Self {
			id,
			ok: false,
			value: Json::Null,
			error: Some(reason.into()),
		}
	}
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	/// The shapes the Swift and Kotlin agents parse by hand: a round trip
	/// proves the names and the casing they see.
	#[test]
	fn instructions_round_trip_through_the_names_agents_parse() {
		let instruction = Instruction::Op {
			id: 7,
			session: 3,
			op: Op::Press {
				key: "b".to_owned(),
				shift: false,
				ctrl: true,
				alt: false,
				meta: false,
			},
		};

		let wire = serde_json::to_value(&instruction).expect("serializes");
		assert_eq!(
			wire,
			json!({
				"kind": "op",
				"id": 7,
				"session": 3,
				"op": {
					"type": "press",
					"key": "b",
					"shift": false,
					"ctrl": true,
					"alt": false,
					"meta": false,
				}
			}),
		);

		let back: Instruction = serde_json::from_value(wire).expect("deserializes");
		assert_eq!(back, instruction);
	}

	#[test]
	fn every_op_round_trips() {
		let ops = [
			Op::Load {
				format: "markdown".to_owned(),
				content: "# Hi".to_owned(),
			},
			Op::TypeText {
				text: "ab".to_owned(),
			},
			Op::InsertText {
				text: "cd".to_owned(),
			},
			Op::Press {
				key: "Enter".to_owned(),
				shift: true,
				ctrl: false,
				alt: true,
				meta: false,
			},
			Op::Select {
				anchor: Position::new([0], 1),
				head: Some(Position::new([1, 0, 0], 2)),
			},
			Op::SelectAll,
			Op::DeleteSelection,
			Op::Paste {
				format: "html".to_owned(),
				content: "<p>x</p>".to_owned(),
			},
			Op::Run {
				command: "toggleBold".to_owned(),
				args: json!(null),
			},
			Op::Can {
				command: "setHeading".to_owned(),
				args: json!({ "level": 2 }),
			},
			Op::Query {
				query: "isBold".to_owned(),
				args: json!(null),
			},
			Op::Undo,
			Op::Redo,
			Op::SetMarkedText {
				text: "こん".to_owned(),
				selected: (0, 1),
			},
			Op::UnmarkText,
			Op::Share { client: 9 },
			Op::ApplyUpdate {
				update: "AA==".to_owned(),
			},
			Op::TakeUpdates,
			Op::EncodeState,
		];

		for op in ops {
			let wire = serde_json::to_value(&op).expect("serializes");
			let back: Op = serde_json::from_value(wire.clone()).expect("deserializes");

			assert_eq!(back, op, "{wire} did not round trip");
		}
	}

	#[test]
	fn start_carries_what_an_agent_builds_an_editor_from() {
		let instruction = Instruction::Start {
			id: 1,
			session: 0,
			extensions: vec!["bold".to_owned()],
			client: 2,
			apple: true,
			load: Load {
				format: "yjs".to_owned(),
				content: "AA==".to_owned(),
			},
		};

		let wire = serde_json::to_value(&instruction).expect("serializes");
		assert_eq!(wire["kind"], "start");
		assert_eq!(wire["load"]["format"], "yjs");
		assert!(wire["apple"].as_bool().unwrap_or_default());
	}

	#[test]
	fn wait_and_done_carry_no_id_to_answer() {
		assert_eq!(Instruction::Wait.id(), None);
		assert_eq!(Instruction::Done.id(), None);
		assert_eq!(Instruction::Close { id: 5, session: 1 }.id(), Some(5));
	}

	#[test]
	fn reads_serialize_to_their_names() {
		assert_eq!(
			serde_json::to_value(Read::CanUndo).unwrap(),
			json!("canUndo")
		);
		assert_eq!(
			serde_json::to_value(Read::Selection).unwrap(),
			json!("selection")
		);
	}

	#[test]
	fn replies_carry_their_error_only_when_failed() {
		let wire = serde_json::to_value(Reply::ok(3, json!(true))).unwrap();

		assert_eq!(wire, json!({ "id": 3, "ok": true, "value": true }));
		assert!(serde_json::to_value(Reply::failed(4, "why")).unwrap()["error"].is_string());
	}
}
