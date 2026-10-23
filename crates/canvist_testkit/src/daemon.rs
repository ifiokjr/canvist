//! The daemon: one process per checkout and target that owns the agent
//! server and the device run.
//!
//! Test runners start a process per test (cargo-nextest) or per binary
//! (cargo test), so the server cannot live in the test process: each test
//! would start its own simulator run. Instead, the first harness for a
//! target spawns the `canvist_testkit_daemon` binary detached; every
//! harness in the checkout — this binary, the next, a parallel one —
//! talks to it. The daemon exits by itself after `CANVIST_TESTKIT_IDLE`
//! seconds (default 600) with no work at all; a session waiting for its
//! agent keeps it up for as long as the device run takes.
//!
//! The daemon serves two callers:
//!
//! - harnesses, with `POST /session`, `POST /session/N/op`, `.../read`,
//!   and `.../close`, which queue instructions and wait for their replies;
//! - one agent at a time, with `GET /next` and `POST /reply`, which
//!   executes instructions against a real editor: the loopback agent in
//!   this process for [`Target::Loopback`], an `XCTest` run on a simulator
//!   for [`Target::Ios`], and a Gradle instrumentation run on an emulator
//!   for [`Target::Android`].

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use serde_json::Value as Json;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::oneshot;

use crate::Target;
use crate::TestResult;
use crate::TestkitError;
use crate::httpio;
use crate::wire::Instruction;
use crate::wire::Load;
use crate::wire::Reply;

/// How long an agent's `GET /next` holds before answering `wait`.
const POLL_HOLD: Duration = Duration::from_secs(5);

/// How long after the daemon starts before an idle `GET /next` may answer
/// `done`, so a slow first harness does not end the run before it begins.
const DONE_GRACE: Duration = Duration::from_secs(30);

/// A handle on the running daemon for one target.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Daemon {
	/// Where the daemon listens.
	pub port: u16,
}

impl Daemon {
	/// The daemon for `target`, starting one if none is running.
	///
	/// # Errors
	///
	/// The daemon could not start, or started but never answered.
	pub(crate) async fn for_target(target: Target) -> TestResult<Self> {
		let root = workspace_root();
		let directory = registry_dir(&root);
		std::fs::create_dir_all(&directory).map_err(|error| {
			TestkitError::unavailable(
				target,
				format!("could not create {}: {error}", directory.display()),
			)
		})?;

		if let Some(daemon) = Self::running(&directory, target).await {
			return Ok(daemon);
		}

		Self::spawn(target, &root, &directory).await
	}

	/// The daemon's registry entry, from a port file that answers.
	async fn running(directory: &Path, target: Target) -> Option<Self> {
		let file = directory.join(format!("{}.port", target.name()));
		let port: u16 = std::fs::read_to_string(file).ok()?.trim().parse().ok()?;
		let ping = httpio::request(port, "GET", "/ping", None, Duration::from_secs(2))
			.await
			.ok()?;

		(ping["target"] == target.name()).then_some(Self { port })
	}

	/// Start the daemon binary detached and wait for it to report its port.
	async fn spawn(target: Target, root: &Path, directory: &Path) -> TestResult<Self> {
		let exe = daemon_binary(target, root)?;
		let port_file = directory.join(format!("{}.port", target.name()));

		let mut command = tokio::process::Command::new(&exe);
		command
			.arg("daemon")
			.arg("--target")
			.arg(target.name())
			.arg("--root")
			.arg(root)
			.arg("--idle-secs")
			.arg(idle_secs().to_string())
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(
				std::fs::File::create(directory.join(format!("{}.daemon.log", target.name())))
					.map_or(Stdio::null(), Stdio::from),
			);

		#[cfg(unix)]
		{
			// The daemon outlives the test process; a new process group
			// keeps a crashing runner from taking it down.
			command.process_group(0);
		}

		#[cfg(not(unix))]
		{
			let _ = &mut command;
		}

		let mut child = command.spawn().map_err(|error| {
			TestkitError::unavailable(
				target,
				format!(
					"could not start the testkit daemon from {}: {error}",
					exe.display()
				),
			)
		})?;

		let first_line = child.stdout.take().map(|stdout| {
			tokio::spawn(async move {
				let mut lines = BufReader::new(stdout).lines();
				lines.next_line().await.ok().flatten()
			})
		});

		let port: u16 = match first_line {
			Some(task) => {
				tokio::time::timeout(Duration::from_secs(30), task)
					.await
					.map_err(|_| {
						TestkitError::unavailable(target, "the daemon did not start in 30s")
					})?
					.map_err(|error| TestkitError::unavailable(target, error.to_string()))?
					.and_then(|line| {
						line.strip_prefix("ready ")
							.and_then(|port| port.parse().ok())
					})
					.ok_or_else(|| {
						TestkitError::unavailable(target, "the daemon did not report its port")
					})?
			}
			None => {
				return Err(TestkitError::unavailable(
					target,
					"the daemon started without a pipe to read",
				));
			}
		};

		std::fs::write(&port_file, port.to_string()).map_err(|error| {
			TestkitError::unavailable(target, format!("could not write the port file: {error}"))
		})?;

		Ok(Self { port })
	}
}

/// The daemon binary to spawn: `CANVIST_TESTKIT_DAEMON`, or the one this
/// checkout built, in the profile this build runs.
fn daemon_binary(target: Target, root: &Path) -> TestResult<PathBuf> {
	if let Some(path) = std::env::var_os("CANVIST_TESTKIT_DAEMON") {
		return Ok(PathBuf::from(path));
	}

	let profile = if cfg!(debug_assertions) {
		"debug"
	} else {
		"release"
	};
	let built = root
		.join("target")
		.join(profile)
		.join("canvist_testkit_daemon");

	if built.is_file() {
		return Ok(built);
	}

	Err(TestkitError::unavailable(
		target,
		format!(
			"the daemon binary is not built at {}; run `cargo build -p canvist_testkit --bin \
			 canvist_testkit_daemon` or set CANVIST_TESTKIT_DAEMON",
			built.display(),
		),
	))
}

/// The workspace root this crate was built in, where the platform
/// directories and the daemon registry live.
pub(crate) fn workspace_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(2)
		.map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Where per-target port files live: inside `target/`, which nothing
/// commits and everything may write.
fn registry_dir(root: &Path) -> PathBuf {
	root.join("target").join("canvist-testkit")
}

/// `CANVIST_TESTKIT_IDLE`, how long the daemon waits with no work at all
/// before exiting. The default outlives a whole test binary; a session
/// waiting for its agent keeps the daemon up however long the device run
/// takes.
fn idle_secs() -> u64 {
	std::env::var("CANVIST_TESTKIT_IDLE")
		.ok()
		.and_then(|value| value.parse().ok())
		.unwrap_or(600)
}

/// One instruction and where its reply goes.
struct Pending {
	instruction: Instruction,
	reply: oneshot::Sender<Json>,
}

/// Everything the connection handlers share.
pub(crate) struct Shared {
	/// Instructions waiting for an agent, oldest first.
	queue: VecDeque<Pending>,
	/// Instructions an agent took, by id, with where their replies go.
	inflight: HashMap<u64, oneshot::Sender<Json>>,
	/// Live sessions, by id.
	sessions: HashSet<u64>,
	/// The next session and instruction ids.
	next_session: u64,
	next_id: u64,
	/// Whether an agent may still come: polling, or a runner bringing one
	/// up.
	agent_live: bool,
	/// The runner's generation, so a late exit does not clobber a newer
	/// runner's state.
	generation: u64,
	/// Woken when the queue grows, a session ends, or an agent exits.
	notify: Notify,
	/// When the daemon started and last served anything.
	started: Instant,
	last_served: Instant,
}

impl Shared {
	fn new() -> Self {
		Self {
			queue: VecDeque::new(),
			inflight: HashMap::new(),
			sessions: HashSet::new(),
			next_session: 0,
			next_id: 0,
			agent_live: false,
			generation: 0,
			notify: Notify::new(),
			started: Instant::now(),
			last_served: Instant::now(),
		}
	}

	/// Queue `instruction`, keeping where its reply goes.
	fn queue_instruction(&mut self, instruction: Instruction) -> oneshot::Receiver<Json> {
		let (sender, receiver) = oneshot::channel();
		self.queue.push_back(Pending {
			instruction,
			reply: sender,
		});
		self.notify.notify_one();

		receiver
	}

	/// Take the next instruction for an agent, if one is waiting.
	fn take_instruction(&mut self) -> Option<Instruction> {
		let pending = self.queue.pop_front()?;

		if let Some(id) = pending.instruction.id() {
			self.inflight.insert(id, pending.reply);
		}

		Some(pending.instruction)
	}

	/// Deliver an agent's reply to the harness waiting on it.
	fn deliver(&mut self, reply: &Reply) {
		if let Some(sender) = self.inflight.remove(&reply.id) {
			let _ =
				sender.send(serde_json::to_value(reply).unwrap_or_else(|_| json!({ "ok": false })));
		}
	}
}

/// What a connection handler needs.
pub(crate) struct Context {
	/// The target agents serve.
	pub(crate) target: Target,
	/// The workspace root, for runners and logs.
	pub(crate) root: PathBuf,
	/// Where this daemon listens.
	pub(crate) port: u16,
	/// The shared queue and sessions.
	pub(crate) state: Arc<Mutex<Shared>>,
	#[allow(dead_code)]
	pub(crate) idle: Duration,
}

/// Run the daemon: bind, report the port on stdout, serve until idle.
///
/// This is the whole `canvist_testkit_daemon` binary.
///
/// # Errors
///
/// The port could not be bound.
pub async fn run(target: Target, root: PathBuf, idle: Duration) -> std::io::Result<()> {
	let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
	let port = listener.local_addr()?.port();

	let context = Arc::new(Context {
		target,
		root: root.clone(),
		port,
		state: Arc::new(Mutex::new(Shared::new())),
		idle,
	});

	runner::ensure(&context).await;

	println!("ready {port}");

	let directory = registry_dir(&root);
	std::fs::create_dir_all(&directory).ok();
	std::fs::write(
		directory.join(format!("{}.port", target.name())),
		port.to_string(),
	)
	.ok();

	// Exit when nothing has asked for anything lately.
	{
		let context = Arc::clone(&context);
		let port_file = directory.join(format!("{}.port", target.name()));
		tokio::spawn(async move {
			loop {
				tokio::time::sleep(Duration::from_secs(15)).await;

				let shared = context.state.lock().await;

				// Never idle out under work: a session waiting for its
				// agent, an agent owing a reply, or a device run still
				// building — an iOS release build alone takes minutes.
				let working = !shared.sessions.is_empty()
					|| !shared.queue.is_empty()
					|| !shared.inflight.is_empty();

				if !working && shared.last_served.elapsed() > context.idle {
					std::fs::remove_file(&port_file).ok();
					std::process::exit(0);
				}
			}
		});
	}

	loop {
		let (mut stream, _) = listener.accept().await?;
		let context = Arc::clone(&context);

		tokio::spawn(async move {
			let request = httpio::read_request(&mut stream).await;

			if let Err(error) = &request {
				eprintln!("[testkit] unreadable request: {error}");
			}

			if let Ok(served) = request {
				eprintln!(
					"[testkit] request {} {} (body {} bytes)",
					served.method,
					served.path,
					served
						.body
						.as_ref()
						.map_or(0, |body| body.to_string().len())
				);
				let reply = route(&context, served).await;

				if let Some(reply) = reply
					&& let Err(error) = httpio::write_response(&mut stream, 200, &reply).await
				{
					eprintln!("[testkit] unwritable response: {error}");
				}
			}
		});
	}
}

/// Serve one request.
async fn route(context: &Arc<Context>, served: httpio::Served) -> Option<Json> {
	context.state.lock().await.last_served = Instant::now();

	match (served.method.as_str(), served.path.as_str()) {
		("GET", "/ping") => Some(json!({ "target": context.target.name() })),
		("GET", "/next") => next(context, &served.query).await,
		("POST", "/reply") => reply(context, served.body).await,
		("POST", "/session") => start_session(context, served.body).await,
		("POST", path) if path.starts_with("/session/") => session_call(context, &served).await,
		_ => Some(json!({ "ok": false, "error": "unknown route" })),
	}
}

/// The agent's long poll: the next instruction, `wait`, or `done`.
async fn next(context: &Arc<Context>, query: &[(String, String)]) -> Option<Json> {
	let agent = query
		.iter()
		.find(|(name, _)| name == "agent")
		.map_or_else(|| "agent".to_owned(), |(_, value)| value.clone());
	let _ = &agent;

	loop {
		{
			let mut shared = context.state.lock().await;

			if let Some(instruction) = shared.take_instruction() {
				shared.agent_live = true;
				return serde_json::to_value(instruction).ok();
			}

			// No work: finish only once every session is closed and the
			// first harness has had its chance.
			if shared.started.elapsed() > DONE_GRACE && shared.sessions.is_empty() {
				return Some(json!({ "kind": "done" }));
			}
		}

		let shared = context.state.lock().await;
		let notified = shared.notify.notified();

		tokio::select! {
			() = notified => {}
			() = tokio::time::sleep(POLL_HOLD) => {
				return Some(json!({ "kind": "wait" }));
			}
		}
	}
}

/// An agent answered: deliver the reply to the harness waiting on it.
async fn reply(context: &Arc<Context>, body: Option<Json>) -> Option<Json> {
	if let Some(reply) = body.and_then(|body| serde_json::from_value::<Reply>(body).ok()) {
		context.state.lock().await.deliver(&reply);

		return Some(json!({ "ok": true }));
	}

	Some(json!({ "ok": false, "error": "unreadable reply" }))
}

/// A harness starts a session: queue `start`, bring an agent up, and wait
/// out the device run.
async fn start_session(context: &Arc<Context>, body: Option<Json>) -> Option<Json> {
	let request = body.unwrap_or_default();

	let instruction = {
		let mut shared = context.state.lock().await;
		let session = shared.next_session;
		shared.next_session += 1;
		shared.sessions.insert(session);

		let id = shared.next_id;
		shared.next_id += 1;
		Instruction::Start {
			id,
			session,
			extensions: string_list(&request["extensions"]),
			client: request["client"].as_u64().unwrap_or(1) as u32,
			apple: request["apple"].as_bool().unwrap_or(false),
			load: Load {
				format: request["load"]["format"]
					.as_str()
					.unwrap_or("text")
					.to_owned(),
				content: request["load"]["content"]
					.as_str()
					.unwrap_or_default()
					.to_owned(),
			},
		}
	};

	let Instruction::Start { session, .. } = instruction else {
		unreachable!("just built a Start")
	};

	let receiver = context.state.lock().await.queue_instruction(instruction);
	runner::ensure(context).await;

	let answer = await_reply(receiver, startup_timeout(), context.target).await;

	Some(match answer {
		Ok(_) => json!({ "ok": true, "session": session }),
		Err(error) => json!({ "ok": false, "error": error }),
	})
}

/// A harness works a session: an op, a read, or a close.
async fn session_call(context: &Arc<Context>, served: &httpio::Served) -> Option<Json> {
	let parts: Vec<&str> = served.path.trim_matches('/').split('/').collect();
	let (session, what) = (parts.get(1)?.parse::<u64>().ok()?, *parts.get(2)?);
	let body = served.body.clone().unwrap_or(json!({}));

	let build = |id: u64| {
		match what {
			"op" => {
				Some(Instruction::Op {
					id,
					session,
					op: serde_json::from_value(body).ok()?,
				})
			}
			"read" => {
				Some(Instruction::Read {
					id,
					session,
					what: serde_json::from_value(body).ok()?,
				})
			}
			"close" => Some(Instruction::Close { id, session }),
			_ => None,
		}
	};

	let instruction = {
		let mut shared = context.state.lock().await;

		if !shared.sessions.contains(&session) {
			return Some(json!({ "ok": false, "error": "unknown or closed session" }));
		}

		let id = shared.next_id;
		shared.next_id += 1;

		if what == "close" {
			shared.sessions.remove(&session);
			shared.notify.notify_one();
		}

		build(id)?
	};

	let receiver = context.state.lock().await.queue_instruction(instruction);

	match await_reply(receiver, op_timeout(), context.target).await {
		Ok(reply) => Some(reply),
		Err(error) => Some(json!({ "ok": false, "error": error })),
	}
}

/// Wait for the agent's reply to one instruction.
async fn await_reply(
	receiver: oneshot::Receiver<Json>,
	timeout: Duration,
	target: Target,
) -> Result<Json, String> {
	tokio::time::timeout(timeout, receiver)
		.await
		.map_err(|_| {
			format!(
				"the {target} agent did not answer in {}s; the run may have failed — see \
				 target/canvist-testkit/{target}.err.log",
				timeout.as_secs(),
			)
		})?
		.map_err(|_| "the daemon dropped the reply".to_owned())
}

/// `CANVIST_TEST_STARTUP_TIMEOUT`, how long a session's `start` may wait
/// for the device run to come up. The default outlives a first release
/// build for iOS and an emulator boot for Android.
fn startup_timeout() -> Duration {
	Duration::from_secs(
		std::env::var("CANVIST_TEST_STARTUP_TIMEOUT")
			.ok()
			.and_then(|value| value.parse().ok())
			.unwrap_or(900),
	)
}

/// `CANVIST_TEST_OP_TIMEOUT`, how long any one op may take.
fn op_timeout() -> Duration {
	Duration::from_secs(
		std::env::var("CANVIST_TEST_OP_TIMEOUT")
			.ok()
			.and_then(|value| value.parse().ok())
			.unwrap_or(120),
	)
}

/// A JSON array of strings.
fn string_list(value: &Json) -> Vec<String> {
	value
		.as_array()
		.map(|items| {
			items
				.iter()
				.filter_map(Json::as_str)
				.map(str::to_owned)
				.collect()
		})
		.unwrap_or_default()
}

/// The runners: what brings an agent up for each target.
mod runner {
	use super::*;

	/// Bring an agent up unless one is live or on its way.
	pub(super) async fn ensure(context: &Arc<Context>) {
		let generation = {
			let mut shared = context.state.lock().await;

			if shared.agent_live {
				return;
			}

			shared.agent_live = true;
			shared.generation += 1;
			shared.generation
		};

		match context.target {
			Target::Loopback => {
				crate::loopback::spawn(context, generation);
			}
			Target::Ios => spawn_device(context, generation, ios_command),
			Target::Android => spawn_device(context, generation, android_command),
			Target::Headless | Target::Dom => {}
		}
	}

	/// Run a device command, logging to the registry, and mark the agent
	/// gone when it exits.
	fn spawn_device(context: &Arc<Context>, generation: u64, command: fn(&Context) -> Vec<String>) {
		let target = context.target;
		let port = context.port;
		let root = context.root.clone();
		let state = Arc::clone(&context.state);
		let argv = command(context);
		let display = argv.join(" ");

		tokio::spawn(async move {
			eprintln!("[testkit] {target} runner: {display}");

			let output = tokio::process::Command::new(&argv[0])
				.args(&argv[1..])
				.current_dir(&root)
				.env("TESTKIT_ROOT", &root)
				.output()
				.await;

			let mut shared = state.lock().await;
			if shared.generation == generation {
				shared.agent_live = false;
				shared.notify.notify_one();
			}

			let log = registry_dir(&root).join(format!("{target}.log"));
			match output {
				Ok(output) => {
					std::fs::write(&log, &output.stdout).ok();
					std::fs::write(log.with_extension("err.log"), &output.stderr).ok();

					if output.status.success() {
						eprintln!("[testkit] {target} runner finished (daemon port {port})");
					} else {
						eprintln!(
							"[testkit] {target} runner failed; logs in {}",
							log.display()
						);
					}
				}
				Err(error) => {
					eprintln!("[testkit] {target} runner did not start: {error}");
				}
			}
		});
	}

	/// The iOS runner: build the Swift package's FFI, then run only the
	/// agent test on a booted simulator. The simulator shares this
	/// machine's loopback, so the daemon's port needs no forwarding.
	fn ios_command(context: &Context) -> Vec<String> {
		let apple = context.root.join("platforms/apple");
		let script = format!(
			r#"set -euo pipefail
cd "{apple}"
./build.sh
udid="$(./simulator.sh)"
# xcodebuild's TEST_RUNER_ passthrough does not reach Swift Package test
# runners, so the daemon's address also lands where the agent looks.
mkdir -p "$TESTKIT_ROOT/target/canvist-testkit"
printf '%s' "http://127.0.0.1:{port}" > "$TESTKIT_ROOT/target/canvist-testkit/ios.server"
export TEST_RUNER_TESTKIT_SERVER="http://127.0.0.1:{port}"
xcodebuild test \
	-scheme Canvist \
	-only-testing:CanvistTests/TestkitAgentTests \
	-destination "platform=iOS Simulator,id=$udid" \
	-derivedDataPath .build/xcode
"#,
			apple = apple.to_string_lossy(),
			port = context.port,
		);

		vec![
			"/usr/bin/env".to_owned(),
			"bash".to_owned(),
			"-c".to_owned(),
			script,
		]
	}

	/// The Android runner: build the bindings, then run only the agent
	/// instrumentation on an attached or freshly booted emulator, with the
	/// daemon's port reversed so `127.0.0.1` reaches this machine.
	fn android_command(context: &Context) -> Vec<String> {
		let script = format!(
			r#"set -euo pipefail
export ANDROID_HOME="${{ANDROID_HOME:-${{ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}}}"
studio_jdk="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
if [ -z "${{JAVA_HOME:-}}" ] && [ -d "$studio_jdk" ]; then
	export JAVA_HOME="$studio_jdk"
fi
cd "{root}/platforms/android"
./build.sh
./emulator.sh bash -c '\
	"$ANDROID_HOME/platform-tools/adb" reverse tcp:{port} tcp:{port} && \
	./gradlew :canvist:connectedDebugAndroidTest --console=plain \
	-Pandroid.testInstrumentationRunnerArguments.class=io.github.ifiokjr.canvist.TestkitAgentTest \
	-Pandroid.testInstrumentationRunnerArguments.testkitServer=http://127.0.0.1:{port}'
"#,
			root = context.root.to_string_lossy(),
			port = context.port,
		);

		vec![
			"/usr/bin/env".to_owned(),
			"bash".to_owned(),
			"-c".to_owned(),
			script,
		]
	}
}
