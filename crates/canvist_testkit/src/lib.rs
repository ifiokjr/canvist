//! Test canvist editors the way people use them: type, press, paste,
//! select, and read the document back — the same test on every platform.
//!
//! One test, written once in Rust, runs against every target the machine
//! has:
//!
//! | Target | What runs the editor | What it needs |
//! |--------|----------------------|---------------|
//! | `headless` | `canvist_headless`, in this process | nothing |
//! | `loopback` | a daemon's agent, over the device protocol | nothing |
//! | `dom` | the real `@canvist/web` editor, in Chromium, through Playwright, typing real keyboard events | the `dom` feature |
//! | `ios` | the `Canvist` Swift package's FFI editor, in an `XCTest` agent on an iOS Simulator | the `ios` feature, Xcode |
//! | `android` | `canvist-android`'s FFI editor, in an instrumentation agent on an emulator | the `android` feature, the SDK |
//!
//! ```no_run
//! use canvist_headless::Format;
//! use canvist_testkit::EditorHarness;
//! use canvist_testkit::cross_test;
//!
//! cross_test! {
//!     /// Typing a heading rule turns the paragraph into a heading.
//!     heading_input_rule, async fn heading(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
//!         editor.load(Format::Text, "").await?;
//!         editor.type_text("## Notes").await?;
//!
//!         editor.assert_markdown("## Notes").await?;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! Tests default to the headless target so a plain `cargo test` needs
//! nothing but Rust. Ask for more with `CANVIST_TEST_TARGET`:
//!
//! ```text
//! CANVIST_TEST_TARGET=headless,loopback cargo test -p canvist_testkit
//! CANVIST_TEST_TARGET=dom cargo test -p canvist_testkit --features dom
//! CANVIST_TEST_TARGET=all cargo test -p canvist_testkit --features dom,ios,android
//! ```
//!
//! Everything runs headless by default: the browser without a window, the
//! simulator without Simulator.app, the emulator with `-no-window`. Set
//! `CANVIST_TEST_HEADED=1` to watch the browser work.
//!
//! The pieces:
//!
//! - [`EditorHarness`] types and reads, on any target.
//! - [`cross_test!`] runs one test body on every selected target.
//! - [`Target`] and [`Selection`] say where tests run.
//! - [`EditorConfig`] says what the editor is made of, by extension name.
//! - [`transform`] is the other kind of test: quick, synchronous
//!   state-A-to-state-B checks, for when the input path is not the point.

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

mod backend;
mod config;
pub mod daemon;
mod error;
mod httpio;
pub mod loopback;
mod position;
mod target;
mod wire;

pub mod harness;
pub mod transform;

pub use crate::config::EditorConfig;
pub use crate::config::STARTER_KIT;
pub use crate::error::TestResult;
pub use crate::error::TestkitError;
pub use crate::harness::Capability;
pub use crate::harness::EditorHarness;
pub use crate::position::Position;
pub use crate::target::Selection;
pub use crate::target::Target;
pub use crate::wire::Op;

/// Run one test body on every selected target.
///
/// The body is an `async fn` taking `&mut `[`EditorHarness`] and
/// returning [`TestResult`]. Each selected target starts an editor, runs
/// the body, and closes it. A body that returns [`TestkitError::Skip`]
/// counts as skipped on that target, not failed —
/// [`EditorHarness::require`] returns it for capabilities a target lacks.
///
/// Two optional clauses shape the run:
///
/// - `targets = [dom, headless]` runs only those targets, intersected
///   with [`Selection::current`]; without it, every selected target runs.
/// - `config = |config| ...` adjusts the [`EditorConfig`] each target
///   starts from, such as `|config| *config =
///   EditorConfig::with_extensions(&["bold", "history"])`.
///
/// ```no_run
/// use canvist_testkit::EditorHarness;
/// use canvist_testkit::Position;
/// use canvist_testkit::cross_test;
///
/// cross_test! {
///     /// Undo puts a deleted word back.
///     undo_restores_word, async fn undo_word(editor: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
///         editor.load_text("one two").await?;
///         editor.select(&Position::new([0], 4), Some(&Position::new([0], 7))).await?;
///         editor.delete_selection().await?;
///         editor.assert_text("one ").await?;
///
///         editor.undo().await?;
///         editor.assert_text("one two").await?;
///         Ok(())
///     }
/// }
/// ```
#[macro_export]
macro_rules! cross_test {
	(
		$(#[$meta:meta])*
		$name:ident,
		$(targets = [$($target:ident),* $(,)?],)?
		$(config = $config:expr,)?
		async fn $run:ident ($editor:ident : &mut $harness:ty) $(-> $result:ty)? $body:block
	) => {
		$(#[$meta])*
		#[tokio::test]
		async fn $name() {
			async fn $run(
				$editor: &mut $crate::EditorHarness,
			) -> $crate::TestResult<()> $body

			let gate: Vec<String> = [$($(stringify!($target).to_ascii_lowercase()),*)?].into();
			let selection = $crate::Selection::current();

			for target in selection.targets {
				if !gate.is_empty() && !gate.iter().any(|name| name == target.name()) {
					continue;
				}

				let mut config = $crate::EditorConfig::default();
				$($config(&mut config);)?
				let _ = &config;

				match $crate::EditorHarness::start(target, config.clone()).await {
					Ok(mut editor) => {
						let outcome = $run(&mut editor).await;
						editor.close().await.ok();

						if let Err($crate::TestkitError::Skip(reason)) = outcome {
							eprintln!("skipped on {target}: {reason}");

							continue;
						}

						outcome.expect("the test body failed");
					}
					Err($crate::TestkitError::Unavailable { reason, .. })
						if !selection.explicit => {
							eprintln!("skipped on {target}: {reason}");
						}
					Err(error) => {
						panic!("could not start the {target} target: {error}");
					}
				}
			}
		}
	};
}
