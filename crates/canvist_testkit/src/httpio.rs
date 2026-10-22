//! Just enough HTTP for the testkit: a one-request client and a
//! one-request server, both JSON, both `Connection: close`.
//!
//! The daemon serves the agent protocol on `127.0.0.1`; the harness, the
//! loopback agent, and the device agents talk to it with these. Nothing
//! here is general-purpose: bodies are JSON, requests are small, and every
//! connection answers once.

use std::io;
use std::time::Duration;

use serde_json::Value as Json;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// The largest request this accepts: a document to load, not a download.
const MAX_BODY: usize = 64 * 1024 * 1024;

/// One request the server read.
pub(crate) struct Served {
	/// `GET` or `POST`.
	pub method: String,
	/// The path, without its query string.
	pub path: String,
	/// The query string's values, keyed by parameter.
	pub query: Vec<(String, String)>,
	/// The body, parsed when there is one.
	pub body: Option<Json>,
}

/// Read one HTTP request from `stream`.
///
/// # Errors
///
/// The connection closed early, the request is malformed, or the body is
/// not JSON.
pub(crate) async fn read_request(stream: &mut TcpStream) -> io::Result<Served> {
	let mut buffer = Vec::with_capacity(1024);
	let mut chunk = [0; 8192];
	let header_end = loop {
		if let Some(at) = find_header_end(&buffer) {
			break at;
		}

		let read = tokio::time::timeout(Duration::from_mins(2), stream.read(&mut chunk)).await??;
		if read == 0 {
			return Err(io::Error::new(
				io::ErrorKind::UnexpectedEof,
				"the connection closed before the request ended",
			));
		}

		buffer.extend_from_slice(&chunk[..read]);

		if buffer.len() > MAX_BODY {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"the request is too large",
			));
		}
	};

	let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
	let mut lines = head.split("\r\n");
	let request = lines.next().unwrap_or_default();
	let mut parts = request.split(' ');
	let method = parts.next().unwrap_or_default().to_owned();
	let target = parts.next().unwrap_or_default().to_owned();

	let mut length = 0usize;
	for line in lines {
		let Some((name, value)) = line.split_once(':') else {
			continue;
		};

		if name.trim().eq_ignore_ascii_case("content-length") {
			length = value
				.trim()
				.parse()
				.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
		}
	}

	let mut body = buffer[header_end + 4..].to_vec();
	while body.len() < length {
		let read = tokio::time::timeout(Duration::from_mins(2), stream.read(&mut chunk)).await??;
		if read == 0 {
			return Err(io::Error::new(
				io::ErrorKind::UnexpectedEof,
				"the connection closed before the body ended",
			));
		}

		body.extend_from_slice(&chunk[..read]);
	}

	let (path, query) = match target.split_once('?') {
		Some((path, query)) => (path.to_owned(), parse_query(query)),
		None => (target, Vec::new()),
	};

	Ok(Served {
		method,
		path,
		query,
		body: if body.is_empty() {
			None
		} else {
			Some(
				serde_json::from_slice(&body)
					.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
			)
		},
	})
}

/// Answer with `status` and `json`, then close.
///
/// # Errors
///
/// The socket failed to write.
pub(crate) async fn write_response(
	stream: &mut TcpStream,
	status: u16,
	json: &Json,
) -> io::Result<()> {
	let body = serde_json::to_vec(json).unwrap_or_else(|_| b"{}".to_vec());
	let reason = match status {
		200 => "OK",
		400 => "Bad Request",
		404 => "Not Found",
		_ => "Internal Server Error",
	};

	let head = format!(
		"HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: \
		 {}\r\nConnection: close\r\n\r\n",
		body.len(),
	);

	stream.write_all(head.as_bytes()).await?;
	stream.write_all(&body).await?;
	stream.flush().await
}

/// Call `method path?query` on `port` with `body`, reading one JSON reply.
///
/// # Errors
///
/// The daemon is down, the request failed, or the reply is not JSON.
pub(crate) async fn request(
	port: u16,
	method: &str,
	path_and_query: &str,
	body: Option<&Json>,
	timeout: Duration,
) -> io::Result<Json> {
	let future = async {
		let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
		let payload = body
			.map(serde_json::to_vec)
			.transpose()
			.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
			.unwrap_or_default();

		let head = format!(
			"{method} {path_and_query} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: \
			 application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
			payload.len(),
		);

		stream.write_all(head.as_bytes()).await?;
		stream.write_all(&payload).await?;
		stream.flush().await?;

		let mut reply = Vec::new();
		stream.read_to_end(&mut reply).await?;

		let text = String::from_utf8_lossy(&reply);
		let body = text
			.split("\r\n\r\n")
			.nth(1)
			.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no reply body"))?;

		serde_json::from_str(body)
			.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
	};

	tokio::time::timeout(timeout, future)
		.await
		.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "the daemon did not answer"))?
}

/// The `\r\n\r\n` that ends a request's headers, if it has arrived.
fn find_header_end(buffer: &[u8]) -> Option<usize> {
	buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// `a=b&c=d` into pairs, percent-decoding the values.
fn parse_query(query: &str) -> Vec<(String, String)> {
	query
		.split('&')
		.filter_map(|pair| {
			let (name, value) = pair.split_once('=')?;
			Some((decode(name), decode(value)))
		})
		.collect()
}

/// Percent-decode `text`, for the handful of characters a name needs.
fn decode(text: &str) -> String {
	let bytes = text.as_bytes();
	let mut out = Vec::with_capacity(bytes.len());

	let mut at = 0;
	while at < bytes.len() {
		match bytes[at] {
			b'%' if at + 2 < bytes.len() => {
				let hex = std::str::from_utf8(&bytes[at + 1..at + 3]).unwrap_or_default();
				if let Ok(byte) = u8::from_str_radix(hex, 16) {
					out.push(byte);
					at += 3;
					continue;
				}

				out.push(b'%');
				at += 1;
			}
			b'+' => {
				out.push(b' ');
				at += 1;
			}
			byte => {
				out.push(byte);
				at += 1;
			}
		}
	}

	String::from_utf8_lossy(&out).into_owned()
}
