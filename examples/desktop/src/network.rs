//! The connection to a y-websocket server, on its own thread so the window
//! never waits on the network. Binary frames are y-sync messages; the
//! editor reads and writes them, and this only carries them.

use futures_util::SinkExt;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use winit::event_loop::EventLoopProxy;

use crate::AppEvent;
use crate::winit;

/// The sending half of a connection.
pub struct Connection {
	outgoing: mpsc::UnboundedSender<Vec<u8>>,
}

impl Connection {
	/// Connect to `url` (such as `ws://localhost:1234/notes`, where `notes`
	/// is the room) and report what happens through `proxy`.
	pub fn open(url: String, proxy: EventLoopProxy<AppEvent>) -> Self {
		let (outgoing, receiver) = mpsc::unbounded_channel();

		std::thread::spawn(move || {
			let runtime = tokio::runtime::Builder::new_current_thread()
				.enable_all()
				.build();
			let reason = match runtime {
				Ok(runtime) => runtime.block_on(run(&url, receiver, &proxy)),
				Err(error) => error.to_string(),
			};

			let _ = proxy.send_event(AppEvent::Disconnected(reason));
		});

		Self { outgoing }
	}

	/// Queue y-sync `messages` for the server.
	pub fn send(&self, messages: Vec<Vec<u8>>) {
		for message in messages {
			// A closed channel means the connection ended, which the window
			// already heard about.
			let _ = self.outgoing.send(message);
		}
	}
}

/// Why the connection stopped when the window went away first.
const WINDOW_CLOSED: &str = "the window closed";

/// Carry messages both ways until either side closes. Returns why it
/// stopped.
async fn run(
	url: &str,
	mut outgoing: mpsc::UnboundedReceiver<Vec<u8>>,
	proxy: &EventLoopProxy<AppEvent>,
) -> String {
	let socket = match tokio_tungstenite::connect_async(url).await {
		Ok((socket, _)) => socket,
		Err(error) => return format!("could not connect to {url}: {error}"),
	};

	let (mut write, mut read) = socket.split();

	if proxy.send_event(AppEvent::Connected).is_err() {
		return WINDOW_CLOSED.to_owned();
	}

	loop {
		let step = tokio::select! {
			message = outgoing.recv() => send(&mut write, message).await,
			frame = read.next() => receive(frame, proxy),
		};

		if let Err(reason) = step {
			return reason;
		}
	}
}

/// Send one message from the window, or stop when the window has gone.
async fn send<S>(socket: &mut S, message: Option<Vec<u8>>) -> Result<(), String>
where
	S: SinkExt<Message> + Unpin,
	S::Error: std::fmt::Display,
{
	let message = message.ok_or(WINDOW_CLOSED)?;
	socket
		.send(Message::Binary(message.into()))
		.await
		.map_err(|error| error.to_string())
}

/// Hand one frame from the server to the window. y-sync travels in binary
/// frames, so text and pings are skipped.
fn receive(
	frame: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
	proxy: &EventLoopProxy<AppEvent>,
) -> Result<(), String> {
	match frame {
		Some(Ok(Message::Binary(bytes))) => {
			proxy
				.send_event(AppEvent::Sync(bytes.to_vec()))
				.map_err(|_| WINDOW_CLOSED.to_owned())
		}
		Some(Ok(Message::Close(_))) | None => Err("the server closed the connection".to_owned()),
		Some(Ok(_)) => Ok(()),
		Some(Err(error)) => Err(error.to_string()),
	}
}
