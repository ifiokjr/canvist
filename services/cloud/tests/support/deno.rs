//! Real JavaScript Yjs clients, run under Deno from `tests/js/client.ts`.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;

use serde_json::Value as Json;
use serde_json::json;

/// How a client connects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
	/// `y-websocket`'s `WebsocketProvider`.
	YWebsocket,
	/// `@hocuspocus/provider`'s `HocuspocusProvider`.
	Hocuspocus,
}

/// The result of connecting.
#[derive(Debug, PartialEq, Eq)]
pub struct Connected {
	pub synced: bool,
	pub denied: Option<String>,
}

/// What a client sees after waiting.
#[derive(Debug, PartialEq, Eq)]
pub struct Seen {
	pub texts: Vec<String>,
	pub peers: u64,
}

/// One JavaScript client process. Calls block, so tests run on the
/// multi-threaded runtime and wrap them in `block_in_place`.
pub struct Client {
	child: Child,
	stdin: ChildStdin,
	stdout: BufReader<ChildStdout>,
}

impl Client {
	/// Start a client. Requires `deno` on the `PATH`, which the devenv shell
	/// provides; the first run downloads the npm packages.
	pub fn spawn() -> Self {
		let script = format!("{}/tests/js/client.ts", env!("CARGO_MANIFEST_DIR"));
		let mut child = Command::new("deno")
			.args([
				"run",
				"--quiet",
				"--no-prompt",
				"--allow-env",
				"--allow-net",
				&script,
			])
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.expect("deno runs; the devenv shell provides it");
		let stdin = child.stdin.take().expect("piped stdin");
		let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
		Self {
			child,
			stdin,
			stdout,
		}
	}

	fn send(&mut self, message: &Json) -> Vec<Json> {
		tokio::task::block_in_place(|| {
			writeln!(self.stdin, "{message}").expect("the client reads stdin");
			let mut replies = Vec::new();

			loop {
				let mut line = String::new();
				let read = self
					.stdout
					.read_line(&mut line)
					.expect("the client writes stdout");
				assert!(read > 0, "the client exited early");
				let reply: Json = serde_json::from_str(&line).expect("one JSON message per line");

				if reply.get("done").is_some() {
					return replies;
				}

				replies.push(reply);
			}
		})
	}

	/// Connect to `url` and wait for the first sync.
	pub fn connect(
		&mut self,
		transport: Transport,
		url: &str,
		document: &str,
		token: &str,
	) -> Connected {
		let transport = match transport {
			Transport::YWebsocket => "y-websocket",
			Transport::Hocuspocus => "hocuspocus",
		};
		let replies = self.send(&json!({
			"connect": { "transport": transport, "url": url, "document": document, "token": token },
		}));
		let reply = &replies[0];
		Connected {
			synced: reply["synced"].as_bool().unwrap_or(false),
			denied: reply["denied"].as_str().map(str::to_owned),
		}
	}

	/// Append a paragraph holding `text`.
	pub fn append(&mut self, text: &str) {
		self.send(&json!({ "append": text }));
	}

	/// Publish a user named `name`.
	pub fn presence(&mut self, name: &str) {
		self.send(&json!({ "presence": name }));
	}

	/// Wait until the document holds `texts` and `peers` others are present.
	pub fn wait(&mut self, texts: &[&str], peers: u64) -> Seen {
		let replies = self.send(&json!({ "wait": { "texts": texts, "peers": peers } }));
		Self::seen(&replies[0])
	}

	/// Wait until the document holds `texts`, ignoring presence.
	pub fn wait_texts(&mut self, texts: &[&str]) -> Vec<String> {
		let replies = self.send(&json!({ "wait": { "texts": texts } }));
		Self::seen(&replies[0]).texts
	}

	fn seen(reply: &Json) -> Seen {
		Seen {
			texts: reply["texts"]
				.as_array()
				.into_iter()
				.flatten()
				.filter_map(|text| text.as_str().map(str::to_owned))
				.collect(),
			peers: reply["peers"].as_u64().unwrap_or(0),
		}
	}

	/// Close the connection.
	pub fn disconnect(&mut self) {
		self.send(&json!({ "disconnect": true }));
	}
}

impl Drop for Client {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}
