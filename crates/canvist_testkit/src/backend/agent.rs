//! The agent backend: the harness's client to the daemon, for the
//! loopback, iOS, and Android targets.
//!
//! Every harness call becomes one wire op posted to the daemon's session
//! endpoint, which queues it for whatever agent the target runs. See
//! `daemon.rs` for the other side.

use base64::Engine as _;
use canvist_headless::Format;
use canvist_headless::canvist_core::Modifiers;
use canvist_headless::canvist_core::Platform;
use serde_json::Value as Json;

/// The base64 the wire protocol uses.
const BASE64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
use serde_json::json;

use crate::EditorConfig;
use crate::Position;
use crate::Target;
use crate::TestResult;
use crate::TestkitError;
use crate::daemon::Daemon;
use crate::httpio;
use crate::wire::Op;
use crate::wire::Read;

/// One session with one agent, through the daemon.
pub(crate) struct AgentBackend {
	target: Target,
	daemon: Daemon,
	session: u64,
	platform: Platform,
}

impl AgentBackend {
	pub(crate) async fn start(target: Target, config: &EditorConfig) -> TestResult<Self> {
		let daemon = Daemon::for_target(target).await?;
		let platform = match target {
			Target::Ios => Platform::Apple,
			_ => config.platform.unwrap_or(Platform::Other),
		};

		let request = json!({
			"extensions": config.extensions,
			"client": config.client,
			"apple": matches!(platform, Platform::Apple),
			"load": { "format": "text", "content": "" },
		});

		let reply = call(
			target,
			daemon.port,
			"POST",
			"/session",
			&request,
			startup_timeout(),
		)
		.await?;

		let session = reply["session"].as_u64().ok_or_else(|| {
			TestkitError::bridge(target, reply["error"].as_str().unwrap_or("no session id"))
		})?;

		Ok(Self {
			target,
			daemon,
			session,
			platform,
		})
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn join(
		target: Target,
		config: &EditorConfig,
		state: &str,
	) -> TestResult<Self> {
		let daemon = Daemon::for_target(target).await?;
		let platform = match target {
			Target::Ios => Platform::Apple,
			_ => config.platform.unwrap_or(Platform::Other),
		};

		let request = json!({
			"extensions": config.extensions,
			"client": config.client,
			"apple": matches!(platform, Platform::Apple),
			"load": { "format": "yjs", "content": state },
		});

		let reply = call(
			target,
			daemon.port,
			"POST",
			"/session",
			&request,
			startup_timeout(),
		)
		.await?;

		let session = reply["session"].as_u64().ok_or_else(|| {
			TestkitError::bridge(target, reply["error"].as_str().unwrap_or("no session id"))
		})?;

		Ok(Self {
			target,
			daemon,
			session,
			platform,
		})
	}

	pub(crate) fn platform(&self) -> Platform {
		self.platform
	}

	#[allow(clippy::unused_self, reason = "every backend answers the same way")]
	pub(crate) fn supports(&self, capability: crate::Capability) -> bool {
		match capability {
			crate::Capability::Ime => true,
			#[cfg(feature = "collab")]
			crate::Capability::Collab => true,
			#[cfg(not(feature = "collab"))]
			crate::Capability::Collab => false,
		}
	}

	pub(crate) async fn load(&mut self, format: Format, content: &str) -> TestResult<()> {
		self.run_quiet(Op::Load {
			format: format.name().to_owned(),
			content: content.to_owned(),
		})
		.await
	}

	pub(crate) async fn type_text(&mut self, text: &str) -> TestResult<bool> {
		self.run_flag(Op::TypeText {
			text: text.to_owned(),
		})
		.await
	}

	pub(crate) async fn insert_text(&mut self, text: &str) -> TestResult<bool> {
		self.run_flag(Op::InsertText {
			text: text.to_owned(),
		})
		.await
	}

	pub(crate) async fn press(&mut self, key: &str, modifiers: Modifiers) -> TestResult<bool> {
		self.run_flag(Op::Press {
			key: key.to_owned(),
			shift: modifiers.shift,
			ctrl: modifiers.ctrl,
			alt: modifiers.alt,
			meta: modifiers.meta,
		})
		.await
	}

	pub(crate) async fn select(
		&mut self,
		anchor: &Position,
		head: Option<&Position>,
	) -> TestResult<()> {
		self.run_quiet(Op::Select {
			anchor: anchor.clone(),
			head: head.cloned(),
		})
		.await
	}

	pub(crate) async fn select_all(&mut self) -> TestResult<()> {
		self.run_quiet(Op::SelectAll).await
	}

	pub(crate) async fn delete_selection(&mut self) -> TestResult<bool> {
		self.run_flag(Op::DeleteSelection).await
	}

	pub(crate) async fn paste(&mut self, format: Format, content: &str) -> TestResult<bool> {
		self.run_flag(Op::Paste {
			format: format.name().to_owned(),
			content: content.to_owned(),
		})
		.await
	}

	pub(crate) async fn run_command(&mut self, command: &str, args: &Json) -> TestResult<bool> {
		self.run_flag(Op::Run {
			command: command.to_owned(),
			args: args.clone(),
		})
		.await
	}

	pub(crate) async fn can_command(&mut self, command: &str, args: &Json) -> TestResult<bool> {
		self.run_flag(Op::Can {
			command: command.to_owned(),
			args: args.clone(),
		})
		.await
	}

	pub(crate) async fn query(&mut self, query: &str, args: &Json) -> TestResult<Json> {
		let reply = self
			.run_op(Op::Query {
				query: query.to_owned(),
				args: args.clone(),
			})
			.await?;

		Ok(reply)
	}

	pub(crate) async fn undo(&mut self) -> TestResult<bool> {
		self.run_flag(Op::Undo).await
	}

	pub(crate) async fn redo(&mut self) -> TestResult<bool> {
		self.run_flag(Op::Redo).await
	}

	pub(crate) async fn compose(&mut self, text: &str) -> TestResult<()> {
		self.run_quiet(Op::SetMarkedText {
			text: text.to_owned(),
			selected: (0, 0),
		})
		.await
	}

	pub(crate) async fn commit_composition(&mut self, text: &str) -> TestResult<()> {
		// The input-method shape every platform follows: replace the
		// marked text, then commit it as it stands.
		self.compose(text).await?;
		self.run_quiet(Op::UnmarkText).await
	}

	pub(crate) async fn cancel_composition(&mut self) -> TestResult<()> {
		self.run_quiet(Op::SetMarkedText {
			text: String::new(),
			selected: (0, 0),
		})
		.await
	}

	pub(crate) async fn read(&mut self, what: Read) -> TestResult<Json> {
		let request = serde_json::to_value(what)
			.map_err(|error| TestkitError::protocol(self.target, error.to_string()))?;

		if !request.is_string() {
			return Err(TestkitError::protocol(
				self.target,
				"a read must serialize to its name",
			));
		}

		let reply = self.call("read", &request).await?;

		if reply["ok"].as_bool().unwrap_or(false) {
			Ok(reply["value"].clone())
		} else {
			Err(TestkitError::bridge(
				self.target,
				reply["error"].as_str().unwrap_or("the read failed"),
			))
		}
	}

	pub(crate) async fn close(&mut self) -> TestResult<()> {
		let request = json!({});
		let reply = self.call("close", &request).await?;

		let _ = reply;

		Ok(())
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn share(&mut self, client_id: u64) -> TestResult<()> {
		self.run_quiet(Op::Share { client: client_id }).await
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn apply_update(&mut self, update: &[u8]) -> TestResult<bool> {
		self.run_flag(Op::ApplyUpdate {
			update: BASE64.encode(update),
		})
		.await
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn take_updates(&mut self) -> TestResult<Vec<Vec<u8>>> {
		let updates = self.run_op(Op::TakeUpdates).await?;
		let updates = updates.as_array().ok_or_else(|| {
			TestkitError::protocol(self.target, "takeUpdates did not return a list")
		})?;

		updates
			.iter()
			.map(|update| {
				update.as_str().ok_or_else(|| {
					TestkitError::protocol(self.target, "an update was not base64 text")
				})
			})
			.map(|text| {
				text.and_then(|text| {
					BASE64.decode(text).map_err(|error| {
						TestkitError::protocol(
							self.target,
							format!("an update was not base64: {error}"),
						)
					})
				})
			})
			.collect()
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn encode_state(&mut self) -> TestResult<Vec<u8>> {
		let state = self.run_op(Op::EncodeState).await?;
		let state = state.as_str().ok_or_else(|| {
			TestkitError::protocol(self.target, "encodeState did not return base64 text")
		})?;

		base64::engine::general_purpose::STANDARD
			.decode(state)
			.map_err(|error| {
				TestkitError::protocol(self.target, format!("the state was not base64: {error}"))
			})
	}

	/// Run one op and return the reply's `value`.
	async fn run_op(&mut self, op: Op) -> TestResult<Json> {
		let request = serde_json::to_value(&op)
			.map_err(|error| TestkitError::protocol(self.target, error.to_string()))?;
		let reply = self.call("op", &request).await?;

		if !reply["ok"].as_bool().unwrap_or(false) {
			let error = reply["error"].as_str().unwrap_or("the op failed");
			let message = match &op {
				Op::Run { command, .. } | Op::Can { command, .. } => {
					format!("`{command}` failed: {error}")
				}
				Op::Query { query, .. } => format!("`{query}` failed: {error}"),
				_ => error.to_owned(),
			};

			return Err(TestkitError::bridge(self.target, message));
		}

		Ok(reply["value"].clone())
	}

	async fn run_flag(&mut self, op: Op) -> TestResult<bool> {
		let value = self.run_op(op).await?;

		value.as_bool().ok_or_else(|| {
			TestkitError::protocol(self.target, "the op did not answer true or false")
		})
	}

	async fn run_quiet(&mut self, op: Op) -> TestResult<()> {
		self.run_op(op).await.map(|_| ())
	}

	async fn call(&mut self, what: &str, request: &Json) -> TestResult<Json> {
		call(
			self.target,
			self.daemon.port,
			"POST",
			&format!("/session/{}/{}", self.session, what),
			request,
			op_timeout(),
		)
		.await
	}
}

/// One call on the daemon, checked for the reply's `ok`.
async fn call(
	target: Target,
	port: u16,
	method: &str,
	path: &str,
	request: &Json,
	timeout: std::time::Duration,
) -> TestResult<Json> {
	let reply = httpio::request(port, method, path, Some(request), timeout)
		.await
		.map_err(|error| {
			TestkitError::bridge(target, format!("the daemon did not answer {path}: {error}"))
		})?;

	Ok(reply)
}

/// `CANVIST_TEST_STARTUP_TIMEOUT` in seconds, matching the daemon's.
fn startup_timeout() -> std::time::Duration {
	std::time::Duration::from_secs(
		std::env::var("CANVIST_TEST_STARTUP_TIMEOUT")
			.ok()
			.and_then(|value| value.parse().ok())
			.unwrap_or(900),
	)
}

/// `CANVIST_TEST_OP_TIMEOUT` in seconds, matching the daemon's.
fn op_timeout() -> std::time::Duration {
	std::time::Duration::from_secs(
		std::env::var("CANVIST_TEST_OP_TIMEOUT")
			.ok()
			.and_then(|value| value.parse().ok())
			.unwrap_or(120),
	)
}
