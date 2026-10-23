# canvist_testkit

Cross-platform integration tests for the canvist editor: one test, written once in Rust, run against every platform the machine has.

Text editors cannot afford to lose content. These tests type the way a person types — one character at a time, real key presses — and read the document back, so a change that breaks one platform breaks a test before it breaks a user:

```rust
use canvist_headless::Format;
use canvist_testkit::EditorHarness;
use canvist_testkit::cross_test;

cross_test! {
	/// Typing a heading rule turns the paragraph into a heading.
	heading_input_rule, async fn heading(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.load(Format::Text, "").await?;
		editor.type_text("## Notes").await?;

		editor.assert_markdown("## Notes").await?;
		Ok(())
	}
}
```

## Targets

`CANVIST_TEST_TARGET` picks where tests run: a comma list, `all`, or nothing (the headless engine, always available).

| Target     | What runs the editor                                                                        | Needs                          |
| ---------- | ------------------------------------------------------------------------------------------- | ------------------------------ |
| `headless` | `canvist_headless`, in this process                                                         | nothing                        |
| `loopback` | a daemon's agent, over the device protocol                                                  | nothing                        |
| `dom`      | the real `@canvist/web` editor, in Chromium through Playwright, typing real keyboard events | the `dom` feature              |
| `ios`      | the `Canvist` Swift package's FFI editor, in an XCTest agent on an iOS Simulator            | the `ios` feature, Xcode       |
| `android`  | `canvist-android`'s FFI editor, in an instrumentation agent on an emulator                  | the `android` feature, the SDK |

```sh
cargo test -p canvist_testkit                                  # headless
CANVIST_TEST_TARGET=headless,loopback cargo test -p canvist_testkit
CANVIST_TEST_TARGET=dom cargo test -p canvist_testkit --features dom
CANVIST_TEST_TARGET=all cargo test -p canvist_testkit --features dom,ios,android,collab
```

A named target must be able to run; an unavailable one fails the test with the fix. Swept targets (`all`) skip with a note, the way the conformance fixtures skip. Everything runs headless by default — the browser without a window, the simulator without Simulator.app, the emulator with `-no-window` — and `CANVIST_TEST_HEADED=1` opens the browser to watch.

A test restricts itself with `targets = [headless, dom]`, and asks what a target can do with `editor.require(Capability::Ime)?`, which skips where the capability is missing instead of failing.

## How the targets work

- **headless** drives a `canvist_headless::Doc` directly. Typing runs one character at a time with a realistic clock, so undo groups a word the way it does on a real host.
- **dom** builds the `@canvist/web` editor in a real browser page (`packages/canvist-web/tests/pages/testkit.ts`). Typing and key presses are real browser keyboard events through the editor's own input layer; loads, selects, commands, and reads go through the page's bridge. The first run builds what the page needs (the headless package's wasm and `dist`, the web package's `node_modules` and wasm); everything already built is skipped, and `CANVIST_TEST_PREP=skip` skips all of it.
- **loopback** runs the same editor in a separate daemon process, reached through the same HTTP instruction stream the device agents speak — the protocol's own test, with no device.
- **ios** and **android** run each platform's real FFI editor — the same API an app calls — inside the platform's own test runner, on a simulator or emulator. The agents (`TestkitAgentTests.swift`, `TestkitAgentTest.kt`) pull instructions from the daemon and answer; `share`, `applyUpdate`, and friends cross the wire as base64, so a collaborative test can have a headless peer and an iOS peer in one document.

The daemon (`canvist_testkit_daemon`) owns the instruction queue and the device run for one checkout and target, shared by every test binary, so cargo-nextest's process-per-test does not start a simulator per test. It exits by itself after `CANVIST_TESTKIT_IDLE` seconds (default 300) idle.

Device runs are slow to start (a release build for iOS, an emulator boot for Android), so their tests live in one test binary and each `start` waits up to `CANVIST_TEST_STARTUP_TIMEOUT` seconds (default 900) for the first agent to come up. A device run that ends while work is still waiting — a failed build, a crashed agent — fails that work at once, pointing at the run's logs under `target/canvist-testkit/`.

## The other kind of test

[`transform`](src/transform.rs) cases are the quick ones: state A to state B, synchronously, in this process. They never leave Rust, so hundreds of small editor rules cost nothing:

```rust
use canvist_testkit::Position;
use canvist_testkit::transform::TransformCase;

TransformCase::new("bold a word")
    .text("bold me")
    .select(&Position::new([0], 0), &Position::new([0], 7))
    .press("Mod-b")
    .expect_markdown("**bold me**")
    .run();
```

## Capabilities, honestly

The targets do not pretend to be identical. The browser types real text but synthesizes no composition events; neither mobile binding pastes by format; collaboration runs on `headless`, `loopback`, `ios`, and `android`. `require` and `targets = [...]` say what a test needs, and the skip notes say why a target sat out.
