//! WebSocket plumbing shared by both transports.

use std::time::Duration;
use std::time::Instant;

use axum::extract::WebSocketUpgrade;
use axum::extract::ws::CloseFrame;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use futures_util::SinkExt;
use futures_util::stream::SplitSink;

use super::room::Limits;
use crate::app::App;

/// How often the server pings an idle connection.
pub const PING_INTERVAL: Duration = Duration::from_secs(25);
/// How often quiet and paused sessions are sent a keepalive, inside the 30
/// seconds after which y-websocket and Hocuspocus clients reconnect.
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);
/// Silence after which a connection is considered dead.
pub const DEAD_AFTER: Duration = Duration::from_secs(70);

/// Close codes the relay uses.
pub mod codes {
	/// The document was deleted.
	pub const NORMAL: u16 = 1000;
	/// The peer sent something unreadable.
	pub const INVALID: u16 = 1007;
	/// The relay could not load the document.
	pub const INTERNAL: u16 = 1011;
	/// The server is restarting; reconnect.
	pub const RESTART: u16 = 1012;
	/// The peer fell too far behind; reconnect to resync.
	pub const TRY_AGAIN: u16 = 1013;
}

/// The sending half of a socket.
pub type Sink = SplitSink<WebSocket, Message>;

/// Send `bytes` as a binary frame. Returns whether the socket is still
/// open.
pub async fn send(sink: &mut Sink, bytes: impl Into<axum::body::Bytes>) -> bool {
	sink.send(Message::Binary(bytes.into())).await.is_ok()
}

/// Close with `code` and `reason`.
pub async fn close(sink: &mut Sink, code: u16, reason: &'static str) {
	let frame = CloseFrame {
		code,
		reason: reason.into(),
	};
	let _ = sink.send(Message::Close(Some(frame))).await;
	let _ = sink.close().await;
}

/// Silence toward a peer after which it gets a keepalive. With the
/// keepalive check every [`KEEPALIVE_INTERVAL`], a peer never waits more
/// than 25 seconds for a message, inside the 30 seconds after which
/// y-websocket and Hocuspocus clients give up and reconnect.
pub const QUIET_AFTER: Duration = Duration::from_secs(15);

/// Tracks traffic in both directions: silence from the peer means it is
/// dead, silence toward it means it needs a keepalive.
#[derive(Debug)]
pub struct Liveness {
	last_seen: Instant,
	last_sent: Instant,
}

impl Default for Liveness {
	fn default() -> Self {
		Self {
			last_seen: Instant::now(),
			last_sent: Instant::now(),
		}
	}
}

impl Liveness {
	/// Note that the peer sent something.
	pub fn seen(&mut self) {
		self.last_seen = Instant::now();
	}

	/// Note that the peer was sent something.
	pub fn sent(&mut self) {
		self.last_sent = Instant::now();
	}

	/// Whether the peer has been sent nothing for [`QUIET_AFTER`].
	pub fn is_quiet(&self) -> bool {
		self.last_sent.elapsed() >= QUIET_AFTER
	}

	/// Whether the peer has been silent too long.
	pub fn is_dead(&self) -> bool {
		self.last_seen.elapsed() > DEAD_AFTER
	}
}

/// Ping, or report that the peer is dead.
pub async fn ping(sink: &mut Sink, liveness: &Liveness) -> bool {
	!liveness.is_dead()
		&& sink
			.send(Message::Ping(axum::body::Bytes::new()))
			.await
			.is_ok()
}

/// Buffer sizes for each socket. tungstenite defaults to 128 KiB each way,
/// which would make buffers, not documents, the cost of a connection.
/// y-sync messages are small, so 8 KiB batches them well.
const BUFFER_BYTES: usize = 8 * 1024;

/// Apply the relay's message limit and buffer sizes to an upgrade.
pub fn configure(upgrade: WebSocketUpgrade, app: &App) -> WebSocketUpgrade {
	let max_message = app.config.sync.max_message_bytes;
	upgrade
		.max_message_size(max_message)
		.read_buffer_size(BUFFER_BYTES)
		.write_buffer_size(BUFFER_BYTES)
		.max_write_buffer_size(max_message.saturating_mul(4))
}

/// The update limits from the configuration.
pub fn limits(app: &App) -> Limits {
	Limits {
		max_update_bytes: app.config.sync.max_message_bytes,
		max_document_bytes: app.config.sync.max_document_bytes,
	}
}
