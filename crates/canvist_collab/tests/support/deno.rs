//! Driving the JavaScript test scripts in `tests/js` under Deno.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;

use serde_json::Value as Json;

/// A running script that reads one JSON message per line on stdin and
/// answers each with zero or more JSON lines, then `{ "done": true }`.
pub struct Script {
	child: Child,
	stdin: ChildStdin,
	stdout: BufReader<ChildStdout>,
}

impl Script {
	/// Start `tests/js/<name>`. Requires `deno` on the `PATH`, which the
	/// devenv shell provides; the first run downloads npm packages.
	pub fn spawn(name: &str) -> Self {
		let script = format!("{}/tests/js/{name}", env!("CARGO_MANIFEST_DIR"));
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

	/// Send one message and return everything the script said before
	/// acknowledging it.
	pub fn send(&mut self, message: &Json) -> Vec<Json> {
		writeln!(self.stdin, "{message}").expect("the script reads stdin");
		let mut replies = Vec::new();

		loop {
			let mut line = String::new();
			let read = self
				.stdout
				.read_line(&mut line)
				.expect("the script writes stdout");
			assert!(read > 0, "the script exited early");
			let reply: Json = serde_json::from_str(&line).expect("one JSON message per line");

			if reply.get("done").is_some() {
				return replies;
			}

			replies.push(reply);
		}
	}

	/// The values of `key` in `replies`, decoded from hex.
	pub fn binary(replies: &[Json], key: &str) -> Vec<Vec<u8>> {
		replies
			.iter()
			.filter_map(|reply| reply.get(key)?.as_str())
			.map(from_hex)
			.collect()
	}
}

impl Drop for Script {
	fn drop(&mut self) {
		// The script ends when its stdin closes; reap it either way.
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

/// Lowercase hex, as the scripts expect binary payloads.
pub fn to_hex(bytes: &[u8]) -> String {
	const DIGITS: &[u8; 16] = b"0123456789abcdef";

	bytes
		.iter()
		.flat_map(|byte| [byte >> 4, byte & 0xf])
		.map(|digit| char::from(DIGITS[usize::from(digit)]))
		.collect()
}

fn from_hex(hex: &str) -> Vec<u8> {
	(0..hex.len())
		.step_by(2)
		.map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex"))
		.collect()
}
