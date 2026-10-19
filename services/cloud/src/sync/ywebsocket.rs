//! The y-websocket transport: `GET /v1/sync/{document}?token=…`.
//!
//! A stock `WebsocketProvider` connects with
//! `new WebsocketProvider("wss://…/v1/sync", "team/notes", doc, { params: { token } })`.
//! Each binary frame is one or more y-protocols messages, and the server
//! speaks first with its state vector and the room's presence, exactly as
//! the reference y-websocket server does.

use std::sync::Arc;

use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use axum::response::Response;
use futures_util::StreamExt;
use futures_util::stream::SplitStream;
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::watch;

use super::protocol;
use super::room::Fanout;
use super::session::Reply;
use super::session::Session;
use super::socket;
use super::socket::Liveness;
use super::socket::Sink;
use super::socket::codes;
use crate::app::SharedApp;
use crate::auth;
use crate::auth::AuthError;
use crate::auth::Principal;
use crate::auth::Scope;
use crate::error::ApiError;
use crate::metering::Admission;
use crate::names::DocumentKey;
use crate::names::DocumentName;

/// The query string of a sync connection.
#[derive(Debug, Deserialize)]
pub struct SyncQuery {
	token: Option<String>,
}

/// Authenticate, then upgrade to a y-websocket session.
pub async fn connect(
	State(app): State<SharedApp>,
	Path(document): Path<String>,
	Query(query): Query<SyncQuery>,
	headers: HeaderMap,
	upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
	if app.is_shutting_down() {
		return Err(shutting_down());
	}

	let credential = query
		.token
		.or_else(|| {
			headers
				.get(AUTHORIZATION)
				.and_then(|value| value.to_str().ok())
				.and_then(auth::bearer)
				.map(str::to_owned)
		})
		.ok_or(AuthError::Missing)?;
	let principal = app.authenticate(&credential).await?;
	let name = DocumentName::parse(&document)?;
	let scope = principal.scope_for(&name).ok_or(AuthError::Forbidden)?;
	let key = DocumentKey {
		project: principal.project.clone(),
		environment: principal.environment.clone(),
		name,
	};
	Ok(socket::configure(upgrade, &app)
		.on_upgrade(move |socket| run(app, socket, key, principal, scope)))
}

pub(super) fn shutting_down() -> ApiError {
	ApiError::new(
		axum::http::StatusCode::SERVICE_UNAVAILABLE,
		"shutting_down",
		"the server is restarting; reconnect",
	)
}

async fn run(
	app: SharedApp,
	socket: WebSocket,
	key: DocumentKey,
	principal: Principal,
	scope: Scope,
) {
	let _alive = app.alive();
	let (mut sink, mut stream) = socket.split();
	let mut shutdown = app.shutdown();
	let paused = match wait_for_admission(&app, &key, &mut sink, &mut stream, &mut shutdown).await {
		Ok(paused) => paused,
		Err(end) => {
			end.close(&mut sink).await;
			return;
		}
	};
	let joined = app.rooms.join(&key).await;
	let Ok((room, membership)) = joined.inspect_err(|error| {
		tracing::error!(document = %key, %error, "could not load document");
	}) else {
		socket::close(&mut sink, codes::INTERNAL, "could not load the document").await;
		return;
	};
	let session = Session::new(
		room,
		membership.peer,
		scope,
		Arc::from(principal.subject()),
		socket::limits(&app),
	);
	let connection = Connection {
		session,
		sink,
		liveness: Liveness::default(),
	};
	connection
		.serve(stream, membership.receiver, shutdown, paused)
		.await;
}

/// What a paused client is told, once. y-websocket logs it as a warning.
const PAUSED: &str = "canvist: this project's free plan has reached its monthly active document \
                      limit; this document will sync once the limit resets or the plan is upgraded";

/// Hold a session on a new document while its project's free plan is
/// full. The socket stays open and alive, so the client keeps editing
/// locally, and admission is checked again every
/// [`pause_recheck`](crate::config::SyncConfig::pause_recheck). Returns
/// whether the session was paused.
async fn wait_for_admission(
	app: &SharedApp,
	key: &DocumentKey,
	sink: &mut Sink,
	stream: &mut SplitStream<WebSocket>,
	shutdown: &mut watch::Receiver<bool>,
) -> Result<bool, End> {
	let mut paused = false;

	loop {
		match app.admit(key).await {
			Ok(Admission::Admitted) => return Ok(paused),
			Ok(Admission::Paused) => {}
			Err(error) => {
				tracing::error!(document = %key, %error, "could not check usage");
				return Err(End::Close(codes::INTERNAL, "could not check usage"));
			}
		}

		if !paused {
			paused = true;
			send_or_gone(sink, protocol::permission_denied(PAUSED)).await?;
		}

		hold(sink, stream, shutdown, app.config.sync.pause_recheck).await?;
	}
}

/// Keep a paused socket alive for `duration`, dropping what it sends.
async fn hold(
	sink: &mut Sink,
	stream: &mut SplitStream<WebSocket>,
	shutdown: &mut watch::Receiver<bool>,
	duration: std::time::Duration,
) -> Result<(), End> {
	let deadline = tokio::time::sleep(duration);
	tokio::pin!(deadline);
	let mut keepalive = tokio::time::interval(socket::KEEPALIVE_INTERVAL);

	loop {
		tokio::select! {
			() = &mut deadline => return Ok(()),
			_ = keepalive.tick() => send_or_gone(sink, protocol::keepalive()).await?,
			frame = stream.next() => {
				if matches!(frame, Some(Ok(Message::Close(_)) | Err(_)) | None) {
					return Err(End::Gone);
				}
			}
			_ = shutdown.changed() => return Err(End::Close(codes::RESTART, "server restarting")),
		}
	}
}

async fn send_or_gone(sink: &mut Sink, message: Vec<u8>) -> Result<(), End> {
	socket::send(sink, message)
		.await
		.then_some(())
		.ok_or(End::Gone)
}

struct Connection {
	session: Session,
	sink: Sink,
	liveness: Liveness,
}

/// Why the connection loop ended, and how to close.
enum End {
	Gone,
	Close(u16, &'static str),
}

impl End {
	async fn close(self, sink: &mut Sink) {
		if let Self::Close(code, reason) = self {
			socket::close(sink, code, reason).await;
		}
	}
}

impl Connection {
	/// Greet the peer, then relay until it leaves, the room closes, or the
	/// server shuts down; leave the room whichever way it ends.
	async fn serve(
		mut self,
		stream: SplitStream<WebSocket>,
		fanout: tokio::sync::broadcast::Receiver<Fanout>,
		shutdown: watch::Receiver<bool>,
		paused: bool,
	) {
		let end = match self.greet(paused).await {
			Ok(()) => self.relay(stream, fanout, shutdown).await,
			Err(end) => end,
		};

		end.close(&mut self.sink).await;
		self.session.leave();
	}

	/// Send the room's state vector and presence. A session that was paused
	/// also gets the whole document, because the client's own step 1 was
	/// dropped while it waited.
	async fn greet(&mut self, paused: bool) -> Result<(), End> {
		let mut greeting = self.session.greeting();

		if paused {
			greeting.push(self.session.room().step2(&yrs::StateVector::default()));
		}

		for message in greeting {
			socket::send(&mut self.sink, message)
				.await
				.then_some(())
				.ok_or(End::Gone)?;
		}

		Ok(())
	}

	async fn relay(
		&mut self,
		mut stream: SplitStream<WebSocket>,
		mut fanout: tokio::sync::broadcast::Receiver<Fanout>,
		mut shutdown: watch::Receiver<bool>,
	) -> End {
		let mut ping = tokio::time::interval(socket::PING_INTERVAL);
		ping.tick().await;

		loop {
			let step = tokio::select! {
				frame = stream.next() => self.incoming(frame).await,
				event = fanout.recv() => self.outgoing(event).await,
				_ = ping.tick() => self.ping().await,
				_ = shutdown.changed() => Err(End::Close(codes::RESTART, "server restarting")),
			};

			if let Err(end) = step {
				return end;
			}
		}
	}

	async fn ping(&mut self) -> Result<(), End> {
		socket::ping(&mut self.sink, &self.liveness)
			.await
			.then_some(())
			.ok_or(End::Gone)
	}

	async fn incoming(&mut self, frame: Option<Result<Message, axum::Error>>) -> Result<(), End> {
		self.liveness.seen();

		let bytes = match frame {
			Some(Ok(Message::Binary(bytes))) => bytes,
			Some(Ok(Message::Close(_)) | Err(_)) | None => return Err(End::Gone),
			// Text frames have no meaning in y-websocket; pings and pongs
			// only prove liveness.
			Some(Ok(_)) => return Ok(()),
		};
		let Ok(replies) = self.session.handle(&bytes) else {
			return Err(End::Close(codes::INVALID, "malformed message"));
		};

		// y-websocket peers got step 1 on connect and have no sync status
		// message, so only y-protocols replies go back.
		let messages = replies.into_iter().filter_map(|reply| {
			match reply {
				Reply::Message(message) => Some(message),
				Reply::Step1(_) | Reply::SyncStatus(_) => None,
			}
		});

		for message in messages {
			socket::send(&mut self.sink, message)
				.await
				.then_some(())
				.ok_or(End::Gone)?;
		}

		Ok(())
	}

	async fn outgoing(&mut self, event: Result<Fanout, RecvError>) -> Result<(), End> {
		match event {
			Ok(Fanout::Message { from, .. }) if from == Some(self.session.peer()) => Ok(()),
			Ok(Fanout::Message { message, .. }) => {
				let bytes = axum::body::Bytes::from_owner(message);
				socket::send(&mut self.sink, bytes)
					.await
					.then_some(())
					.ok_or(End::Gone)
			}
			Ok(Fanout::Closed) => Err(End::Close(codes::NORMAL, "document deleted")),
			Err(RecvError::Lagged(_)) => {
				Err(End::Close(codes::TRY_AGAIN, "fell behind; reconnect"))
			}
			Err(RecvError::Closed) => Err(End::Gone),
		}
	}
}
