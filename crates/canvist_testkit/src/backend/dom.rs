//! The dom backend: the real `@canvist/web` editor, in a real browser,
//! through Playwright.
//!
//! A test's typing and key presses are real browser keyboard events, which
//! the page's own input layer handles — the same road a person's keyboard
//! takes. Everything else (loading, selecting, commands, reads) goes
//! through the bridge `tests/pages/testkit.ts` exposes.
//!
//! One browser per test process, one page per harness. The editor runs
//! headless by default; `CANVIST_TEST_HEADED=1` opens a window to watch.
//!
//! The first run builds what the page needs (skipped for anything already
//! built, and everything, with `CANVIST_TEST_PREP=skip`):
//!
//! 1. `@canvist/headless`'s JavaScript and types, which the page imports;
//! 2. the `node_modules` the dev server resolves them through;
//! 3. the `canvist_web` WebAssembly the editor runs.

use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use canvist_headless::Format;
use canvist_headless::canvist_core::Modifiers;
use canvist_headless::canvist_core::Platform;
use playwright_rs::LaunchOptions;
use playwright_rs::Playwright;
use serde_json::Value as Json;
use serde_json::json;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::OnceCell;

use crate::EditorConfig;
use crate::Position;
use crate::Target;
use crate::TestResult;
use crate::TestkitError;
use crate::wire::Read;

/// The dev server: one per test process. The browser cannot join it: a
/// Playwright browser is bound to the tokio runtime that launched it, and
/// every `#[tokio::test]` is its own runtime, so each test thread builds
/// its own browser against this one server.
struct Server {
	port: u16,
	/// Held for its lifetime; the process exit takes it down.
	_vite: Child,
}

static SERVER: OnceCell<Server> = OnceCell::const_new();

// The browser for this thread, which is to say this test runtime.
thread_local! {
	static BROWSER: RefCell<Option<(Playwright, playwright_rs::Browser)>> =
		const { RefCell::new(None) };
}

/// One editor page.
pub(crate) struct DomBackend {
	page: playwright_rs::Page,
	platform: Platform,
}

#[allow(
	clippy::unused_async,
	reason = "the dispatch macro calls every backend the same way"
)]
impl DomBackend {
	pub(crate) async fn start(config: &EditorConfig) -> TestResult<Self> {
		let (port, browser) = lab().await?;

		let mut query = format!(
			"extensions={}&client={}",
			config.extensions.join(","),
			config.client,
		);
		query = query.replace(",client=1", "").replace("extensions=&", "");

		let page = browser
			.new_page()
			.await
			.map_err(|error| TestkitError::bridge(Target::Dom, error.to_string()))?;
		page.goto(
			&format!("http://127.0.0.1:{port}/testkit.html?{query}"),
			None,
		)
		.await
		.map_err(|error| TestkitError::bridge(Target::Dom, error.to_string()))?;

		// The page reports itself when the editor has painted.
		for _ in 0..100 {
			let ready = page
				.evaluate::<Json, bool>("() => globalThis.testkit?.ready === true", None)
				.await
				.unwrap_or(false);

			if ready {
				break;
			}

			tokio::time::sleep(Duration::from_millis(100)).await;
		}

		let modifier = page
			.evaluate::<Json, String>("() => globalThis.testkit.modifier", None)
			.await
			.map_err(|error| TestkitError::bridge(Target::Dom, error.to_string()))?;
		let platform = if modifier == "Meta" {
			Platform::Apple
		} else {
			Platform::Other
		};

		Ok(Self { page, platform })
	}

	pub(crate) fn platform(&self) -> Platform {
		self.platform
	}

	#[allow(clippy::unused_self, reason = "every backend answers the same way")]
	pub(crate) fn supports(&self, capability: crate::Capability) -> bool {
		match capability {
			// Real keyboard events carry no composition state; the editor's
			// EditContext integration owns that path.
			crate::Capability::Ime => false,
			#[cfg(feature = "collab")]
			crate::Capability::Collab => false,
			#[cfg(not(feature = "collab"))]
			crate::Capability::Collab => false,
		}
	}

	pub(crate) async fn load(&mut self, format: Format, content: &str) -> TestResult<()> {
		self.call(
			"load",
			&json!({ "format": format.name(), "content": content }),
		)
		.await
		.map(|_| ())
	}

	pub(crate) async fn type_text(&mut self, text: &str) -> TestResult<bool> {
		self.focus().await?;

		self.page
			.keyboard()
			.type_text(text, None)
			.await
			.map_err(|error| TestkitError::bridge(Target::Dom, error.to_string()))?;

		Ok(true)
	}

	pub(crate) async fn insert_text(&mut self, text: &str) -> TestResult<bool> {
		self.call("insertText", &json!(text))
			.await
			.and_then(bool_of)
	}

	pub(crate) async fn press(&mut self, key: &str, modifiers: Modifiers) -> TestResult<bool> {
		self.focus().await?;

		let chord = [
			modifiers.ctrl.then_some("Control"),
			modifiers.alt.then_some("Alt"),
			modifiers.meta.then_some("Meta"),
			modifiers.shift.then_some("Shift"),
		]
		.into_iter()
		.flatten()
		.chain([key])
		.collect::<Vec<_>>()
		.join("+");

		self.page
			.keyboard()
			.press(&chord, None)
			.await
			.map_err(|error| TestkitError::bridge(Target::Dom, error.to_string()))?;

		Ok(true)
	}

	pub(crate) async fn select(
		&mut self,
		anchor: &Position,
		head: Option<&Position>,
	) -> TestResult<()> {
		self.call(
			"select",
			&json!({ "anchor": anchor, "head": head.unwrap_or(anchor) }),
		)
		.await
		.map(|_| ())
	}

	pub(crate) async fn select_all(&mut self) -> TestResult<()> {
		self.press(
			"a",
			Modifiers {
				meta: self.platform == Platform::Apple,
				..press_defaults()
			},
		)
		.await
		.map(|_| ())
	}

	pub(crate) async fn delete_selection(&mut self) -> TestResult<bool> {
		self.press("Backspace", press_defaults()).await
	}

	pub(crate) async fn paste(&mut self, _format: Format, _content: &str) -> TestResult<bool> {
		Err(TestkitError::bridge(
			Target::Dom,
			"the dom target has no clipboard; test pasting on another target",
		))
	}

	pub(crate) async fn run_command(&mut self, command: &str, args: &Json) -> TestResult<bool> {
		self.call("run", &json!({ "command": command, "args": args }))
			.await
			.and_then(bool_of)
	}

	pub(crate) async fn can_command(&mut self, _command: &str, _args: &Json) -> TestResult<bool> {
		Err(TestkitError::bridge(
			Target::Dom,
			"the page bridge cannot ask `can` yet; use `run`",
		))
	}

	pub(crate) async fn query(&mut self, query: &str, args: &Json) -> TestResult<Json> {
		self.call("query", &json!({ "query": query, "args": args }))
			.await
			.map(|value| value.unwrap_or(Json::Null))
	}

	pub(crate) async fn undo(&mut self) -> TestResult<bool> {
		self.call("undo", &Json::Null).await.and_then(bool_of)
	}

	pub(crate) async fn redo(&mut self) -> TestResult<bool> {
		self.call("redo", &Json::Null).await.and_then(bool_of)
	}

	pub(crate) async fn compose(&mut self, _text: &str) -> TestResult<()> {
		Err(TestkitError::unavailable(
			Target::Dom,
			"the browser target does not synthesize composition events",
		))
	}

	pub(crate) async fn commit_composition(&mut self, _text: &str) -> TestResult<()> {
		Err(TestkitError::unavailable(
			Target::Dom,
			"the browser target does not synthesize composition events",
		))
	}

	pub(crate) async fn cancel_composition(&mut self) -> TestResult<()> {
		Err(TestkitError::unavailable(
			Target::Dom,
			"the browser target does not synthesize composition events",
		))
	}

	pub(crate) async fn read(&mut self, what: Read) -> TestResult<Json> {
		let op = match what {
			Read::Text => self.call("export", &json!("text")).await,
			Read::Markdown => self.call("export", &json!("markdown")).await,
			Read::Html => self.call("export", &json!("html")).await,
			Read::Json => self.call("json", &Json::Null).await,
			Read::Selection => self.call("selection", &Json::Null).await,
			Read::CanUndo => {
				Err(TestkitError::bridge(
					Target::Dom,
					"`canUndo` is not in the page bridge yet",
				))
			}
			Read::CanRedo => {
				Err(TestkitError::bridge(
					Target::Dom,
					"`canRedo` is not in the page bridge yet",
				))
			}
		}?;

		Ok(op.unwrap_or(Json::Null))
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn share(&mut self, _client_id: u64) -> TestResult<()> {
		Err(TestkitError::bridge(
			Target::Dom,
			"the dom target does not collaborate yet",
		))
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn apply_update(&mut self, _update: &[u8]) -> TestResult<bool> {
		Err(TestkitError::bridge(
			Target::Dom,
			"the dom target does not collaborate yet",
		))
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn take_updates(&mut self) -> TestResult<Vec<Vec<u8>>> {
		Err(TestkitError::bridge(
			Target::Dom,
			"the dom target does not collaborate yet",
		))
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn encode_state(&mut self) -> TestResult<Vec<u8>> {
		Err(TestkitError::bridge(
			Target::Dom,
			"the dom target does not collaborate yet",
		))
	}

	pub(crate) async fn close(&mut self) -> TestResult<()> {
		self.page
			.close()
			.await
			.map_err(|error| TestkitError::bridge(Target::Dom, error.to_string()))
	}

	/// Give the editor focus the way a click does, so keyboard events
	/// reach it.
	async fn focus(&mut self) -> TestResult<()> {
		self.call("focus", &Json::Null).await.map(|_| ())
	}

	/// One bridge op, checked for its `ok`.
	async fn call(&mut self, op: &str, argument: &Json) -> TestResult<Option<Json>> {
		let expression = "(args) => globalThis.testkit.op(args[0], args[1])";
		let answer = self
			.page
			.evaluate::<Json, Json>(expression, Some(&json!([op, argument])))
			.await
			.map_err(|error| TestkitError::bridge(Target::Dom, error.to_string()))?;

		if answer["ok"].as_bool().unwrap_or(false) {
			Ok(answer.get("value").cloned())
		} else {
			Err(TestkitError::bridge(
				Target::Dom,
				answer["error"].as_str().unwrap_or("the op failed"),
			))
		}
	}
}

/// No modifiers held.
fn press_defaults() -> Modifiers {
	Modifiers {
		shift: false,
		ctrl: false,
		alt: false,
		meta: false,
	}
}

/// An answer's boolean value.
fn bool_of(value: Option<Json>) -> TestResult<bool> {
	value
		.and_then(|value| value.as_bool())
		.ok_or_else(|| TestkitError::protocol(Target::Dom, "the op did not answer a boolean"))
}

/// The dev server and this thread's browser: prepare the page's inputs,
/// start the server once, and give this runtime a browser.
async fn lab() -> TestResult<(u16, playwright_rs::Browser)> {
	let server = SERVER
		.get_or_init(|| {
			async {
				start_server().await.expect(
					"the dom server could not start; see CANVIST_TEST_PREP to skip preparation",
				)
			}
		})
		.await;

	let browser = browser().await?;

	Ok((server.port, browser))
}

/// This thread's browser, launched once per runtime.
async fn browser() -> TestResult<playwright_rs::Browser> {
	let present = BROWSER.with(|slot| slot.borrow().is_some());

	if !present {
		let headed = std::env::var_os("CANVIST_TEST_HEADED").is_some_and(|value| !value.is_empty());
		let playwright = Playwright::launch()
			.await
			.map_err(|error| TestkitError::unavailable(Target::Dom, error.to_string()))?;
		let options = LaunchOptions::default().headless(!headed);
		let browser = launch_chromium(&playwright, options.clone()).await?;

		BROWSER.with(|slot| *slot.borrow_mut() = Some((playwright, browser)));
	}

	BROWSER
		.with(|slot| slot.borrow().as_ref().map(|(_, browser)| browser.clone()))
		.ok_or_else(|| TestkitError::bridge(Target::Dom, "this thread lost its browser"))
}

/// Start the dev server, reporting failures as testkit errors.
async fn start_server() -> TestResult<Server> {
	let root = crate::daemon::workspace_root();
	let web = web_directory(&root);

	prepare(&root, &web).await?;

	let port = free_port().await;

	// The package's own vite, not `npx`: package-manager shims daemonize on
	// some machines, and a dev server that outlives its child confuses the
	// health check. Its output lands in the log the error message names.
	let vite_log = root.join("target/canvist-testkit/dom.vite.log");
	let vite = Command::new(web.join("node_modules/.bin/vite"))
		.arg("--config")
		.arg("tests/vite.config.ts")
		.arg("--port")
		.arg(port.to_string())
		.arg("--strictPort")
		// Vite binds ::1 alone by default; the health check and the page
		// both speak IPv4.
		.arg("--host")
		.arg("127.0.0.1")
		.current_dir(&web)
		.stdout(std::process::Stdio::from(
			std::fs::File::create(&vite_log)
				.map_err(|error| TestkitError::unavailable(Target::Dom, error.to_string()))?,
		))
		.stderr(std::process::Stdio::from(
			std::fs::File::create(&vite_log)
				.map_err(|error| TestkitError::unavailable(Target::Dom, error.to_string()))?,
		))
		.kill_on_drop(true)
		.spawn()
		.map_err(|error| {
			TestkitError::unavailable(
				Target::Dom,
				format!("could not start the vite dev server: {error}"),
			)
		})?;

	// Wait for the page to be served.
	let url = format!("http://127.0.0.1:{port}/testkit.html");
	let mut up = false;
	for _ in 0..180 {
		if tokio::net::TcpStream::connect(("127.0.0.1", port))
			.await
			.is_ok()
		{
			up = true;

			break;
		}

		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	if !up {
		return Err(TestkitError::unavailable(
			Target::Dom,
			format!("the vite dev server never served {url}"),
		));
	}

	Ok(Server { port, _vite: vite })
}

/// Launch chromium, fetching the driver's own copy first when this machine
/// has none.
async fn launch_chromium(
	playwright: &Playwright,
	options: LaunchOptions,
) -> TestResult<playwright_rs::Browser> {
	match playwright
		.chromium()
		.launch_with_options(options.clone())
		.await
	{
		Ok(browser) => Ok(browser),
		Err(first) if first.to_string().contains("is not installed") => {
			eprintln!("[testkit] dom prep: installing Playwright's chromium");

			playwright_rs::install_browsers(Some(&["chromium"]))
				.await
				.map_err(|error| {
					TestkitError::unavailable(
						Target::Dom,
						format!("chromium was missing and installing it failed: {error}"),
					)
				})?;

			playwright
				.chromium()
				.launch_with_options(options)
				.await
				.map_err(|error| {
					TestkitError::unavailable(
						Target::Dom,
						format!("{first}; installed and still failing: {error}"),
					)
				})
		}
		Err(first) => Err(TestkitError::unavailable(Target::Dom, first.to_string())),
	}
}

/// Build what the page needs, skipping whatever is already there.
async fn prepare(root: &Path, web: &Path) -> TestResult<()> {
	if std::env::var_os("CANVIST_TEST_PREP").is_some_and(|value| value == "skip") {
		return Ok(());
	}

	let headless = root.join("packages/canvist-headless");

	if !headless.join("dist").is_dir() {
		npm(&headless, &["ci"]).await?;
		// The package's own wasm modules must exist before its TypeScript
		// compiles, which imports them.
		npm(&headless, &["run", "build:wasm"]).await?;
		npm(&headless, &["run", "build"]).await?;
	}

	if !web.join("node_modules").is_dir() {
		npm(web, &["ci"]).await?;
	}

	if !web.join("wasm/canvist_web.js").is_file() {
		run(
			Target::Dom,
			"wasm-pack",
			&[
				"build",
				"crates/canvist_web",
				"--target",
				"web",
				"--profile",
				"wasm-web",
				"--no-opt",
				"--out-dir",
				"../../packages/canvist-web/wasm",
				"--no-pack",
			],
			root,
			Some("--cfg canvist_omit_docs"),
		)
		.await?;
	}

	Ok(())
}

/// Run `npm arguments` in `directory`.
async fn npm(directory: &Path, arguments: &[&str]) -> TestResult<()> {
	run(Target::Dom, "npm", arguments, directory, None).await
}

/// Run one preparation command, failing with its tail.
async fn run(
	target: Target,
	program: &str,
	arguments: &[&str],
	directory: &Path,
	rustflags: Option<&str>,
) -> TestResult<()> {
	let mut command = Command::new(program);
	command.args(arguments).current_dir(directory);

	if let Some(flags) = rustflags {
		command.env("RUSTFLAGS", flags);
	}

	eprintln!("[testkit] {target} prep: {program} {}", arguments.join(" "));

	let output = tokio::time::timeout(Duration::from_mins(20), command.output())
		.await
		.map_err(|_| {
			TestkitError::Tool {
				target,
				tool: "prep",
				stderr: format!(
					"`{program} {}` did not finish in 20 minutes",
					arguments.join(" ")
				),
			}
		})?
		.map_err(|error| {
			TestkitError::Tool {
				target,
				tool: "prep",
				stderr: format!("`{program} {}` failed: {error}", arguments.join(" ")),
			}
		})?;

	if output.status.success() {
		return Ok(());
	}

	// Tools disagree about which stream carries their errors.
	let output_text = format!(
		"{}{}",
		String::from_utf8_lossy(&output.stderr),
		String::from_utf8_lossy(&output.stdout),
	);
	let lines: Vec<&str> = output_text.lines().collect();
	let tail = lines[lines.len().saturating_sub(40)..].join("\n");

	Err(TestkitError::Tool {
		target,
		tool: "prep",
		stderr: format!("`{program} {}` failed:\n{tail}", arguments.join(" ")),
	})
}

/// The `@canvist/web` package directory: `CANVIST_WEB_DIR` for a prepared
/// one, or this checkout's.
fn web_directory(root: &Path) -> PathBuf {
	std::env::var_os("CANVIST_WEB_DIR")
		.map_or_else(|| root.join("packages/canvist-web"), PathBuf::from)
}

/// A port the dev server can bind.
async fn free_port() -> u16 {
	tokio::net::TcpListener::bind(("127.0.0.1", 0))
		.await
		.and_then(|listener| listener.local_addr())
		.map_or(5180, |address| address.port())
}
