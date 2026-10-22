# Testing

Read this page to test a canvist editor the way a person uses it: type, press, paste, select, and read the document back — in Rust, on every platform the editor ships to.

The framework is `canvist_testkit`, a crate in this repository. One test, written once, runs against every target the machine has:

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

## Why integration first

A text editor cannot afford to lose content. A unit test that calls a transformation directly proves the transformation; it does not prove the keyboard reached it, that the browser's input layer translated the event the way the simulator's did, or that undo still groups a word typed fast. The testkit types through the real input path of each platform and reads the state back, so "it works" means a person could have done it.

## Targets

`CANVIST_TEST_TARGET` picks where tests run. Nothing else is needed: tests default to the headless target, which needs only Rust.

| Target     | What runs the editor                                                        | Needs                          |
| ---------- | --------------------------------------------------------------------------- | ------------------------------ |
| `headless` | `canvist_headless`, in this process                                         | nothing                        |
| `loopback` | a daemon's agent, over the same protocol the devices speak                  | nothing                        |
| `dom`      | the real `@canvist/web` editor, in Chromium through Playwright              | the `dom` feature              |
| `ios`      | the `Canvist` Swift package's FFI editor, in an XCTest agent on a Simulator | the `ios` feature, Xcode       |
| `android`  | `canvist-android`'s FFI editor, in an instrumentation agent on an emulator  | the `android` feature, the SDK |

```sh
cargo test -p canvist_testkit                                   # headless
CANVIST_TEST_TARGET=headless,loopback cargo test -p canvist_testkit
CANVIST_TEST_TARGET=dom cargo test -p canvist_testkit --features dom
CANVIST_TEST_TARGET=all cargo test -p canvist_testkit --features dom,ios,android,collab
```

A named target must be able to run, so `CANVIST_TEST_TARGET=ios` fails loudly on a machine without Xcode instead of quietly passing. The sweep (`all`) skips what cannot run with a note, the way the [conformance fixtures](https://github.com/canvist/canvist/blob/main/conformance/readme.md) skip.

Everything runs headless by default: the browser without a window, the simulator without Simulator.app, the emulator with `-no-window`. Set `CANVIST_TEST_HEADED=1` to watch the browser work.

Tests can restrict themselves, because platforms honestly differ:

```rust
cross_test! {
	/// IME composition, on the targets whose input path has it.
	composition_commits, targets = [headless, loopback, ios, android], async fn compose(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		editor.require(canvist_testkit::Capability::Ime)?;
		// ...
		Ok(())
	}
}
```

`require` skips the test on targets without the capability; `targets = [...]` intersects with whatever the run asked for.

## Extensions in combination

An extension proves nothing alone; it proves itself next to the others. The testkit's tests state their extension set, and the same set is registered on every target — by name, the way the JavaScript, Dart, Swift, and Kotlin bindings register them:

```rust
cross_test! {
	/// Two marks on one word: whichever toggles second does not undo the first.
	two_marks_stack,
	config = |config: &mut canvist_testkit::EditorConfig| *config = canvist_testkit::EditorConfig::with_extensions(&["bold", "italic", "history"]),
	async fn stacking(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		// ...
		Ok(())
	}
}
```

## Collaboration

Two editors, one document, in one test — including across targets, so a change to the collaboration layer cannot pass in Rust and fail on a phone:

```rust
# use canvist_testkit::{EditorConfig, EditorHarness, Target};
# async fn example(a: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
let mut phone = EditorHarness::join(Target::Loopback, EditorConfig::default().client(2), &a.encode_state().await?).await?;

a.type_text(" hello").await?;
a.sync_to(&mut phone).await?;

phone.assert_text("base hello").await?;
# Ok(())
# }
```

## Quick transforms, when the input path is not the point

Hundreds of small editor rules — a mark applied to a range, a join across blocks, an input rule firing — do not need a browser. [`TransformCase`](https://github.com/canvist/canvist/blob/main/crates/canvist_testkit/src/transform.rs) runs them synchronously in this process, microseconds a step:

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

When a rule's input path starts to matter, the same steps move up into a `cross_test!` unchanged — the names match on purpose.

## Where to read more

- [`crates/canvist_testkit`](https://github.com/canvist/canvist/blob/main/crates/canvist_testkit) — the framework: harness, targets, daemon, agents.
- [The conformance fixtures](https://github.com/canvist/canvist/blob/main/conformance/readme.md) — the language-neutral document tests every binding runs.
- [Performance](./performance.md) — the budget tests that hold every pull request to its numbers.
