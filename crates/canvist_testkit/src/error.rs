//! Errors a test can meet: everything a harness call returns.

use crate::Target;

/// What went wrong in a [`crate::EditorHarness`] call.
#[derive(Debug, thiserror::Error)]
pub enum TestkitError {
	/// The target asked for is not built into this binary. Rebuild with its
	/// Cargo feature, such as `--features dom`.
	#[error("the {target} target needs the `{feature}` Cargo feature")]
	Feature {
		/// The target that was asked for.
		target: Target,
		/// The feature that adds it.
		feature: &'static str,
	},
	/// The target asked for cannot run here: a tool it needs is missing.
	#[error("the {target} target is unavailable: {reason}")]
	Unavailable {
		/// The target that was asked for.
		target: Target,
		/// What is missing and how to fix it.
		reason: String,
	},
	/// A tool the target runs exited with a failure: the build script, the
	/// emulator, or the browser.
	#[error("the {target} target's `{tool}` failed: {stderr}")]
	Tool {
		/// The target that was asked for.
		target: Target,
		/// What was run.
		tool: &'static str,
		/// Its output, so the fix does not need a rerun.
		stderr: String,
	},
	/// The editor rejected the operation: an invalid position, an unknown
	/// command, a key that does not parse.
	#[error("{target}: {source}")]
	Editor {
		/// The target that was asked for.
		target: Target,
		/// The error the editor reported.
		source: canvist_headless::HeadlessError,
	},
	/// The agent on the device, or the page in the browser, failed in a way
	/// the editor would not: a bridge bug, not an editor bug.
	#[error("{target}: the test bridge failed: {message}")]
	Bridge {
		/// The target that was asked for.
		target: Target,
		/// What the bridge reported.
		message: String,
	},
	/// The harness read the page, agent, or daemon and could not parse what
	/// came back.
	#[error("{target}: unreadable reply from the test bridge: {message}")]
	Protocol {
		/// The target that was asked for.
		target: Target,
		/// What could not be read.
		message: String,
	},
	/// I/O outside a tool run: reading a file, accepting a connection.
	#[error("io: {0}")]
	Io(#[from] std::io::Error),
	/// A test chose to skip: the target lacks what the test exercises.
	///
	/// Returned by [`crate::EditorHarness::require`]; the
	/// [`crate::cross_test!`] macro turns it into a skipped test, not a
	/// failure.
	#[error("skipped: {0}")]
	Skip(String),
}

impl TestkitError {
	/// An error the target's bridge reported.
	#[must_use]
	pub fn bridge(target: Target, message: impl Into<String>) -> Self {
		Self::Bridge {
			target,
			message: message.into(),
		}
	}

	/// An unparseable reply from the target's bridge.
	#[must_use]
	pub fn protocol(target: Target, message: impl Into<String>) -> Self {
		Self::Protocol {
			target,
			message: message.into(),
		}
	}

	/// A missing prerequisite for the target.
	#[must_use]
	pub fn unavailable(target: Target, reason: impl Into<String>) -> Self {
		Self::Unavailable {
			target,
			reason: reason.into(),
		}
	}

	/// The editor's own error, from the target that ran it.
	#[must_use]
	pub fn editor(target: Target, source: canvist_headless::HeadlessError) -> Self {
		Self::Editor { target, source }
	}
}

/// What every harness call returns.
pub type TestResult<T> = Result<T, TestkitError>;
