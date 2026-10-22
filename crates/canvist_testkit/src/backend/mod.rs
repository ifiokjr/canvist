//! The targets' backends: each one runs the same operations a different
//! way.

pub(crate) mod headless;

#[cfg(feature = "dom")]
pub(crate) mod dom;

pub(crate) mod agent;

use canvist_headless::canvist_core::Platform;

use crate::Capability;
use crate::EditorConfig;
use crate::Target;
use crate::TestResult;
use crate::TestkitError;

/// Whichever backend the harness holds. Every arm implements the same
/// methods as [`headless::HeadlessBackend`]; the `dispatch!` macro in
/// `harness.rs` calls through.
#[allow(
	clippy::large_enum_variant,
	reason = "one editor per harness, never in a collection"
)]
pub(crate) enum Backend {
	/// The editor in this process.
	Headless(headless::HeadlessBackend),
	/// The real browser editor, through Playwright.
	#[cfg(feature = "dom")]
	Dom(dom::DomBackend),
	/// The platform FFI editor on a simulator or emulator, through the
	/// testkit agent daemon — and the loopback agent, through the same
	/// protocol in the daemon's own process.
	Agent(agent::AgentBackend),
}

impl Backend {
	/// Start the target's backend and give it an editor.
	pub(crate) async fn start(target: Target, config: &EditorConfig) -> TestResult<Self> {
		match target {
			Target::Headless => {
				Ok(Self::Headless(
					headless::HeadlessBackend::start(config).await?,
				))
			}
			Target::Loopback | Target::Ios | Target::Android => {
				Ok(Self::Agent(
					agent::AgentBackend::start(target, config).await?,
				))
			}
			#[cfg(feature = "dom")]
			Target::Dom => Ok(Self::Dom(dom::DomBackend::start(config).await?)),
			#[cfg(not(feature = "dom"))]
			Target::Dom => Err(missing_feature(target)),
		}
	}

	/// Start the target's backend joined to a shared document (base64 v1
	/// Yjs state), for `collab` tests.
	#[cfg(feature = "collab")]
	pub(crate) async fn join(
		target: Target,
		config: &EditorConfig,
		state: &str,
	) -> TestResult<Self> {
		if matches!(target, Target::Loopback | Target::Ios | Target::Android) {
			return agent::AgentBackend::join(target, config, state)
				.await
				.map(Self::Agent);
		}

		match target {
			Target::Headless => {
				Ok(Self::Headless(
					headless::HeadlessBackend::join(config, state).await?,
				))
			}
			Target::Loopback | Target::Ios | Target::Android => Err(missing_feature(target)),
			#[cfg(feature = "dom")]
			Target::Dom => {
				Err(TestkitError::bridge(
					target,
					"the dom target does not collaborate yet",
				))
			}
			#[cfg(not(feature = "dom"))]
			Target::Dom => Err(missing_feature(target)),
		}
	}

	pub(crate) fn platform(&self) -> Platform {
		match self {
			Self::Headless(backend) => backend.platform(),
			#[cfg(feature = "dom")]
			Self::Dom(backend) => backend.platform(),
			Self::Agent(backend) => backend.platform(),
		}
	}

	/// Whether this backend can do `capability`.
	pub(crate) fn supports(&self, capability: Capability) -> bool {
		match self {
			Self::Headless(backend) => backend.supports(capability),
			#[cfg(feature = "dom")]
			Self::Dom(backend) => backend.supports(capability),
			Self::Agent(backend) => backend.supports(capability),
		}
	}
}

/// The error for a target whose feature is off, which
/// [`Backend::start`] arms without the feature reach.
#[cold]
fn missing_feature(target: Target) -> TestkitError {
	TestkitError::Feature {
		target,
		feature: target.feature(),
	}
}
