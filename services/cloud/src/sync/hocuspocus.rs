//! The Hocuspocus transport: `GET /v1/hocuspocus`.
//!
//! A stock `HocuspocusProvider` connects with
//! `new HocuspocusProvider({ url: "wss://…/v1/hocuspocus", name: "team/notes", token })`.
//! Every frame starts with the document name, so one socket can carry many
//! documents. Each document authenticates separately with an auth message
//! carrying a client token or API key; until then its messages are dropped.
//!
//! On top of y-protocols, Hocuspocus adds `SyncReply` (a sync message that
//! must not be answered with another step 1), `SyncStatus` (whether an
//! update was saved), and `Close`. Stateless messages are accepted and
//! ignored.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::response::Response;
use futures_util::StreamExt;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use yrs::StateVector;
use yrs::encoding::read::Cursor;
use yrs::encoding::read::Read;
use yrs::encoding::write::Write;
use yrs::updates::encoder::Encoder;
use yrs::updates::encoder::EncoderV1;

use super::protocol;
use super::room::Fanout;
use super::session::Reply;
use super::session::Session;
use super::socket;
use super::socket::Liveness;
use super::socket::Sink;
use super::socket::codes;
use crate::app::SharedApp;
use crate::auth::AuthError;
use crate::auth::Scope;
use crate::error::ApiError;
use crate::metering::Admission;
use crate::names::DocumentKey;
use crate::names::DocumentName;

/// Hocuspocus message types.
mod kind {
	pub(super) const SYNC: u8 = 0;
	pub(super) const AWARENESS: u8 = 1;
	pub(super) const AUTH: u8 = 2;
	pub(super) const QUERY_AWARENESS: u8 = 3;
	pub(super) const SYNC_REPLY: u8 = 4;
	pub(super) const STATELESS: u8 = 5;
	pub(super) const CLOSE: u8 = 7;
	pub(super) const SYNC_STATUS: u8 = 8;
	pub(super) const TOKEN: u8 = 0;
	pub(super) const PERMISSION_DENIED: u8 = 1;
	pub(super) const AUTHENTICATED: u8 = 2;
}

/// Documents one socket may have open.
const MAX_DOCUMENTS: usize = 64;
/// Frames queued for a socket before it is disconnected as too slow.
const OUTBOX: usize = 1024;

/// Upgrade to a Hocuspocus session. Authentication happens per document,
/// inside the socket.
pub async fn connect(
	State(app): State<SharedApp>,
	upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
	if app.is_shutting_down() {
		return Err(super::ywebsocket::shutting_down());
	}

	Ok(socket::configure(upgrade, &app).on_upgrade(move |socket| run(app, socket)))
}

/// A document open on the socket.
struct Open {
	session: Session,
	forward: JoinHandle<()>,
}

/// What a token grants for one document.
struct Grant {
	key: DocumentKey,
	scope: Scope,
	editor: Arc<str>,
}

/// A document authenticated but paused because its free plan is full.
struct Paused {
	grant: Grant,
	checked: Instant,
}

/// The stateless message a paused document gets, as JSON. Providers emit it
/// as a `stateless` event.
const PAUSED: &str = r#"{"type":"canvist.paused","reason":"document_limit","message":"this project's free plan has reached its monthly active document limit; this document will sync once the limit resets or the plan is upgraded"}"#;

/// Frames for the socket, from forwarders.
enum Outbox {
	Frame(Vec<u8>),
	Lagged,
}

struct Connection {
	app: SharedApp,
	sink: Sink,
	documents: HashMap<String, Open>,
	paused: HashMap<String, Paused>,
	outbox: mpsc::Sender<Outbox>,
	liveness: Liveness,
}

async fn run(app: SharedApp, socket: WebSocket) {
	let _alive = app.alive();
	let (sink, mut stream) = socket.split();
	let (outbox, mut inbox) = mpsc::channel(OUTBOX);
	let mut shutdown = app.shutdown();
	let mut connection = Connection {
		app,
		sink,
		documents: HashMap::new(),
		paused: HashMap::new(),
		outbox,
		liveness: Liveness::default(),
	};
	let mut ping = tokio::time::interval(socket::PING_INTERVAL);
	ping.tick().await;
	let mut keepalive = tokio::time::interval(socket::KEEPALIVE_INTERVAL);

	let end = loop {
		let step = tokio::select! {
			frame = stream.next() => connection.incoming(frame).await,
			Some(out) = inbox.recv() => connection.outgoing(out).await,
			_ = ping.tick() => connection.ping().await,
			_ = keepalive.tick() => connection.keep_paused().await,
			_ = shutdown.changed() => Err(Some((codes::RESTART, "server restarting"))),
		};

		if let Err(end) = step {
			break end;
		}
	};

	if let Some((code, reason)) = end {
		socket::close(&mut connection.sink, code, reason).await;
	}

	for (_, open) in connection.documents.drain() {
		open.forward.abort();
		open.session.leave();
	}
}

/// `Err(None)` ends the connection silently, `Err(Some(..))` closes it.
type Step = Result<(), Option<(u16, &'static str)>>;

impl Connection {
	async fn ping(&mut self) -> Step {
		// Unauthenticated sockets get one ping interval to authenticate.
		if self.documents.is_empty() && self.paused.is_empty() {
			return Err(Some((codes::NORMAL, "no document authenticated")));
		}

		socket::ping(&mut self.sink, &self.liveness)
			.await
			.then_some(())
			.ok_or(None)
	}

	async fn outgoing(&mut self, out: Outbox) -> Step {
		match out {
			Outbox::Frame(frame) => self.send(frame).await,
			Outbox::Lagged => Err(Some((codes::TRY_AGAIN, "fell behind; reconnect"))),
		}
	}

	async fn send(&mut self, frame: Vec<u8>) -> Step {
		socket::send(&mut self.sink, frame)
			.await
			.then_some(())
			.ok_or(None)
	}

	async fn incoming(&mut self, frame: Option<Result<Message, axum::Error>>) -> Step {
		self.liveness.seen();

		let bytes = match frame {
			Some(Ok(Message::Binary(bytes))) => bytes,
			Some(Ok(Message::Close(_)) | Err(_)) | None => return Err(None),
			Some(Ok(_)) => return Ok(()),
		};

		// One-byte frames are the provider's connection-level ping and pong.
		if bytes.len() <= 1 {
			return Ok(());
		}

		self.dispatch(&bytes).await
	}

	async fn dispatch(&mut self, bytes: &[u8]) -> Step {
		let invalid = Some((codes::INVALID, "malformed message"));
		let mut cursor = Cursor::new(bytes);
		let address = cursor.read_string().map_err(|_| invalid)?.to_owned();
		let message_kind: u8 = cursor.read_var().map_err(|_| invalid)?;
		let rest = &bytes[cursor.next..];

		match message_kind {
			kind::AUTH => self.authenticate(&address, &mut cursor).await,
			kind::SYNC | kind::SYNC_REPLY | kind::AWARENESS | kind::QUERY_AWARENESS => {
				self.relay(&address, message_kind, rest).await
			}
			kind::CLOSE => {
				self.close_document(&address);
				Ok(())
			}
			_ => Ok(()),
		}
	}

	fn close_document(&mut self, address: &str) {
		if let Some(open) = self.documents.remove(address) {
			open.forward.abort();
			open.session.leave();
		}
	}

	/// Hand a y-protocols message to the document's session and send its
	/// replies. Messages for documents that have not authenticated are
	/// dropped, as Hocuspocus drops them.
	async fn relay(&mut self, address: &str, message_kind: u8, rest: &[u8]) -> Step {
		let Some(open) = self.documents.get(address) else {
			return Ok(());
		};
		let replies = open
			.session
			.handle(&y_message(message_kind, rest))
			.map_err(|_| Some((codes::INVALID, "malformed message")))?;
		// A step 1 sent as a `SyncReply` asks for no step 1 in return.
		let answer_step1 = message_kind == kind::SYNC;
		let frames: Vec<Vec<u8>> = replies
			.into_iter()
			.filter_map(|reply| reply_frame(address, reply, answer_step1))
			.collect();

		for frame in frames {
			self.send(frame).await?;
		}

		Ok(())
	}

	async fn authenticate(&mut self, address: &str, cursor: &mut Cursor<'_>) -> Step {
		let invalid = Some((codes::INVALID, "malformed auth message"));
		let auth_kind: u8 = cursor.read_var().map_err(|_| invalid)?;

		if auth_kind != kind::TOKEN {
			return Ok(());
		}

		let token = cursor.read_string().map_err(|_| invalid)?.to_owned();

		match self.open(address, &token).await {
			Ok(scope) => self.authenticated(address, scope).await,
			Err(reason) => {
				let reply = auth_reply(kind::PERMISSION_DENIED, &reason);
				self.send(frame(address, &reply)).await
			}
		}
	}

	/// Tell the peer it may sync `address`, then send the room's presence,
	/// as Hocuspocus does on connect. A paused document is authenticated
	/// too, so the provider stays happy, and told why it is not syncing.
	async fn authenticated(&mut self, address: &str, scope: Scope) -> Step {
		let scope = match scope {
			Scope::Read => "readonly",
			Scope::Write => "read-write",
		};
		self.send(frame(address, &auth_reply(kind::AUTHENTICATED, scope)))
			.await?;

		if self.paused.contains_key(address) {
			let mut notice = vec![kind::STATELESS];
			notice.extend(var_string(PAUSED));
			return self.send(frame(address, &notice)).await;
		}

		let awareness = self
			.documents
			.get(address)
			.and_then(|open| open.session.room().awareness());

		match awareness {
			Some(awareness) => self.send(frame(address, &awareness)).await,
			None => Ok(()),
		}
	}

	/// Keep paused documents alive, and sync any that may now: they get the
	/// whole document and a step 1, because their own step 1 was dropped.
	async fn keep_paused(&mut self) -> Step {
		let recheck = self.app.config.sync.pause_recheck;
		let addresses: Vec<String> = self.paused.keys().cloned().collect();

		for address in addresses {
			self.send(frame(&address, &protocol::keepalive())).await?;
			let due = self
				.paused
				.get(&address)
				.is_some_and(|paused| paused.checked.elapsed() >= recheck);

			if due {
				self.recheck(&address).await?;
			}
		}

		Ok(())
	}

	async fn recheck(&mut self, address: &str) -> Step {
		let Some(paused) = self.paused.remove(address) else {
			return Ok(());
		};

		match self.app.admit(&paused.grant.key).await {
			Ok(Admission::Admitted) => {}
			Ok(Admission::Paused) | Err(_) => {
				self.paused.insert(
					address.to_owned(),
					Paused {
						grant: paused.grant,
						checked: Instant::now(),
					},
				);
				return Ok(());
			}
		}

		if self.join(address, paused.grant).await.is_err() {
			return Err(Some((codes::INTERNAL, "could not load the document")));
		}

		let Some(room) = self
			.documents
			.get(address)
			.map(|open| open.session.room().clone())
		else {
			return Ok(());
		};

		for message in [room.step1(), room.step2(&StateVector::default())] {
			self.send(frame(address, &message)).await?;
		}

		Ok(())
	}

	/// Authenticate `token` for `address` and join its room.
	async fn open(&mut self, address: &str, token: &str) -> Result<Scope, String> {
		if let Some(open) = self.documents.get(address) {
			return Ok(open.session.scope());
		}

		if let Some(paused) = self.paused.get(address) {
			return Ok(paused.grant.scope);
		}

		if self.documents.len() + self.paused.len() >= MAX_DOCUMENTS {
			return Err("too many documents on one connection".to_owned());
		}

		let grant = self.authorize(address, token).await?;
		let scope = grant.scope;
		let admission = self.app.admit(&grant.key).await.map_err(|error| {
			tracing::error!(document = %grant.key, %error, "could not check usage");
			"could not check usage".to_owned()
		})?;

		if admission == Admission::Paused {
			let paused = Paused {
				grant,
				checked: Instant::now(),
			};
			self.paused.insert(address.to_owned(), paused);
			return Ok(scope);
		}

		self.join(address, grant).await?;

		Ok(scope)
	}

	/// Join the room `grant` names and relay it on `address`.
	async fn join(&mut self, address: &str, grant: Grant) -> Result<(), String> {
		let (room, membership) = self.app.rooms.join(&grant.key).await.map_err(|error| {
			tracing::error!(document = %grant.key, %error, "could not load document");
			"could not load the document".to_owned()
		})?;
		let session = Session::new(
			room,
			membership.peer,
			grant.scope,
			grant.editor,
			socket::limits(&self.app),
		);
		let forward = tokio::spawn(forward(
			address.to_owned(),
			membership.peer,
			membership.receiver,
			self.outbox.clone(),
		));
		self.documents
			.insert(address.to_owned(), Open { session, forward });

		Ok(())
	}

	/// The document, scope, and editor name `token` grants for `address`.
	async fn authorize(&self, address: &str, token: &str) -> Result<Grant, String> {
		let principal = self
			.app
			.authenticate(token)
			.await
			.map_err(|_| AuthError::Invalid.to_string())?;
		let name = DocumentName::parse(address).map_err(|error| error.to_string())?;
		let scope = principal
			.scope_for(&name)
			.ok_or_else(|| AuthError::Forbidden.to_string())?;
		let editor = Arc::from(principal.subject());
		let key = DocumentKey {
			project: principal.project,
			environment: principal.environment,
			name,
		};

		Ok(Grant { key, scope, editor })
	}
}

/// The y-protocols message inside a Hocuspocus message of `message_kind`.
/// A `SyncReply` is an ordinary sync message to the session.
fn y_message(message_kind: u8, rest: &[u8]) -> Vec<u8> {
	let y_kind = if message_kind == kind::SYNC_REPLY {
		kind::SYNC
	} else {
		message_kind
	};

	std::iter::once(y_kind)
		.chain(rest.iter().copied())
		.collect()
}

/// The frame carrying `reply`, if Hocuspocus peers expect one.
fn reply_frame(address: &str, reply: Reply, answer_step1: bool) -> Option<Vec<u8>> {
	match reply {
		Reply::Message(message) => Some(frame(address, &message)),
		Reply::Step1(message) => answer_step1.then(|| frame(address, &message)),
		Reply::SyncStatus(saved) => Some(frame(address, &[kind::SYNC_STATUS, u8::from(saved)])),
	}
}

/// An auth message of `auth_kind` carrying `text`.
fn auth_reply(auth_kind: u8, text: &str) -> Vec<u8> {
	let mut reply = vec![kind::AUTH, auth_kind];
	reply.extend(var_string(text));
	reply
}

/// Copy a room's broadcasts for this peer into the socket's outbox.
async fn forward(
	address: String,
	peer: u64,
	mut receiver: tokio::sync::broadcast::Receiver<Fanout>,
	outbox: mpsc::Sender<Outbox>,
) {
	loop {
		let out = match receiver.recv().await {
			Ok(Fanout::Message { from, message }) if from != Some(peer) => {
				Outbox::Frame(frame(&address, &message))
			}
			Ok(Fanout::Message { .. }) => continue,
			Ok(Fanout::Closed) => {
				let reply = auth_reply(kind::PERMISSION_DENIED, "document deleted");
				Outbox::Frame(frame(&address, &reply))
			}
			Err(RecvError::Lagged(_)) => Outbox::Lagged,
			Err(RecvError::Closed) => return,
		};

		if outbox.try_send(out).is_err() {
			let _ = outbox.try_send(Outbox::Lagged);
			return;
		}
	}
}

/// `message` prefixed with the document address.
fn frame(address: &str, message: &[u8]) -> Vec<u8> {
	let mut encoder = EncoderV1::new();
	encoder.write_string(address);
	let mut frame = encoder.to_vec();
	frame.extend_from_slice(message);
	frame
}

fn var_string(value: &str) -> Vec<u8> {
	let mut encoder = EncoderV1::new();
	encoder.write_string(value);
	encoder.to_vec()
}
