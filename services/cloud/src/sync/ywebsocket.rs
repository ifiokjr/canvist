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
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

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
	let (mut sink, stream) = socket.split();

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
		.serve(stream, membership.receiver, app.shutdown())
		.await;
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

impl Connection {
	/// Greet the peer, then relay until it leaves, the room closes, or the
	/// server shuts down; leave the room whichever way it ends.
	async fn serve(
		mut self,
		stream: futures_util::stream::SplitStream<WebSocket>,
		fanout: tokio::sync::broadcast::Receiver<Fanout>,
		shutdown: tokio::sync::watch::Receiver<bool>,
	) {
		let end = match self.greet().await {
			Ok(()) => self.relay(stream, fanout, shutdown).await,
			Err(end) => end,
		};

		if let End::Close(code, reason) = end {
			socket::close(&mut self.sink, code, reason).await;
		}

		self.session.leave();
	}

	async fn greet(&mut self) -> Result<(), End> {
		for message in self.session.greeting() {
			socket::send(&mut self.sink, message)
				.await
				.then_some(())
				.ok_or(End::Gone)?;
		}

		Ok(())
	}

	async fn relay(
		&mut self,
		mut stream: futures_util::stream::SplitStream<WebSocket>,
		mut fanout: tokio::sync::broadcast::Receiver<Fanout>,
		mut shutdown: tokio::sync::watch::Receiver<bool>,
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
