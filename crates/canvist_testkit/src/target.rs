//! Where a test runs: the targets, what each needs, and which were asked
//! for.

use std::fmt;
use std::str::FromStr;

use crate::TestkitError;

/// One place a test can run.
///
/// Every target runs the same test through the same operations; only the
/// road into the editor differs:
///
/// - [`Target::Headless`] runs the editor in this process, with no UI. It
///   is always available, so it is the default.
/// - [`Target::Loopback`] runs the editor in a separate daemon process,
///   through the same HTTP instruction stream the device agents speak.
///   It tests the agent protocol without a device.
/// - [`Target::Dom`] runs the real `@canvist/web` editor in a real
///   browser, through Playwright, typing with real keyboard events.
/// - [`Target::Ios`] runs the `Canvist` Swift package's FFI editor inside
///   an `XCTest` agent on an iOS Simulator.
/// - [`Target::Android`] runs `canvist-android`'s FFI editor inside an
///   instrumentation agent on an Android emulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
	/// In this process, through `canvist_headless`.
	Headless,
	/// A daemon process, through the agent protocol with an in-process
	/// agent.
	Loopback,
	/// A real browser page, through Playwright.
	Dom,
	/// An iOS Simulator, through the Swift package's test agent.
	Ios,
	/// An Android emulator, through the Android test agent.
	Android,
}

impl Target {
	/// Every target this build knows about.
	#[must_use]
	pub fn built() -> &'static [Target] {
		[
			Target::Headless,
			Target::Loopback,
			#[cfg(feature = "dom")]
			Target::Dom,
			#[cfg(feature = "ios")]
			Target::Ios,
			#[cfg(feature = "android")]
			Target::Android,
		]
		.as_slice()
	}

	/// The lower-case name, as `CANVIST_TEST_TARGET` writes it.
	#[must_use]
	pub fn name(self) -> &'static str {
		match self {
			Self::Headless => "headless",
			Self::Loopback => "loopback",
			Self::Dom => "dom",
			Self::Ios => "ios",
			Self::Android => "android",
		}
	}

	/// The Cargo feature that adds this target, if it is not built in.
	#[must_use]
	pub fn feature(self) -> &'static str {
		match self {
			Self::Headless | Self::Loopback => "starter-kit",
			Self::Dom => "dom",
			Self::Ios => "ios",
			Self::Android => "android",
		}
	}

	/// Whether this target is compiled in, feature flags included.
	#[must_use]
	pub fn is_built(self) -> bool {
		match self {
			Self::Headless | Self::Loopback => cfg!(feature = "starter-kit"),
			Self::Dom => cfg!(feature = "dom"),
			Self::Ios => cfg!(feature = "ios"),
			Self::Android => cfg!(feature = "android"),
		}
	}
}

impl fmt::Display for Target {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(self.name())
	}
}

impl FromStr for Target {
	type Err = TestkitError;

	fn from_str(name: &str) -> Result<Self, Self::Err> {
		let name = name.trim().to_ascii_lowercase();
		let target = match name.as_str() {
			"headless" => Self::Headless,
			"loopback" => Self::Loopback,
			"dom" | "browser" | "web" => Self::Dom,
			"ios" | "simulator" => Self::Ios,
			"android" | "emulator" => Self::Android,
			other => {
				return Err(TestkitError::protocol(
					Self::Headless,
					format!("unknown target `{other}`; try headless, dom, ios, or android"),
				));
			}
		};

		if !target.is_built() {
			return Err(TestkitError::Feature {
				target,
				feature: target.feature(),
			});
		}

		Ok(target)
	}
}

/// The `CANVIST_TEST_TARGET` selection: named targets, or everything that
/// can run.
///
/// - Unset: `headless`, so a plain `cargo test` is quick and needs nothing
///   but Rust.
/// - A comma list, such as `dom,ios`: exactly those targets, and each must
///   be able to run — an unavailable one fails the test rather than
///   silently passing.
/// - `all`: every target this build knows about, skipping the ones that
///   cannot run here with a note, the way the conformance fixtures skip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
	/// The targets to run, in order.
	pub targets: Vec<Target>,
	/// Whether the list was written out (`dom,ios`) or swept (`all`, or the
	/// default). Written-out targets must run; swept ones may skip.
	pub explicit: bool,
}

impl Selection {
	/// Read the selection from `CANVIST_TEST_TARGET`.
	///
	/// # Errors
	///
	/// A name that is not a target, or a named target this build lacks the
	/// feature for.
	pub fn from_env() -> Result<Self, TestkitError> {
		Self::parse(std::env::var("CANVIST_TEST_TARGET").ok().as_deref())
	}

	/// Parse `CANVIST_TEST_TARGET`'s value: absent, `all`, or a comma
	/// list.
	///
	/// # Errors
	///
	/// A name that is not a target, or a named target this build lacks the
	/// feature for.
	pub fn parse(value: Option<&str>) -> Result<Self, TestkitError> {
		let Some(value) = value else {
			return Ok(Self {
				targets: vec![Target::Headless],
				explicit: false,
			});
		};

		if value.trim().eq_ignore_ascii_case("all") {
			return Ok(Self {
				targets: Target::built().to_vec(),
				explicit: false,
			});
		}

		let mut targets = Vec::new();
		for name in value.split(',') {
			if name.trim().is_empty() {
				continue;
			}

			let target = Target::from_str(name)?;
			if !targets.contains(&target) {
				targets.push(target);
			}
		}

		if targets.is_empty() {
			return Ok(Self {
				targets: vec![Target::Headless],
				explicit: false,
			});
		}

		Ok(Self {
			targets,
			explicit: true,
		})
	}

	/// The selection every [`crate::cross_test!`] asks for. Invalid
	/// selections fail the test that reads them, so a typo in the
	/// environment never looks like a pass.
	#[must_use]
	pub fn current() -> Self {
		Self::from_env().unwrap_or_else(|error| {
			panic!("CANVIST_TEST_TARGET is not a target list: {error}");
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn absent_selection_is_headless_and_sweepable() {
		let selection = Selection::parse(None).expect("absent parses");

		assert_eq!(selection.targets, [Target::Headless]);
		assert!(!selection.explicit);
	}

	#[test]
	fn blank_selection_falls_back_to_headless() {
		let selection = Selection::parse(Some(" , ")).expect("blank parses");

		assert_eq!(selection.targets, [Target::Headless]);
		assert!(!selection.explicit);
	}

	#[test]
	fn all_sweeps_every_built_target_sweepably() {
		let selection = Selection::parse(Some("all")).expect("all parses");

		assert_eq!(selection.targets, Target::built());
		assert!(!selection.explicit);
	}

	#[test]
	fn named_targets_run_exactly_and_explicitly() {
		let selection = Selection::parse(Some("loopback,headless,loopback")).expect("list parses");

		assert_eq!(selection.targets, [Target::Loopback, Target::Headless]);
		assert!(selection.explicit);
	}

	#[test]
	fn aliases_resolve() {
		// Only names this build carries a feature for parse; the others
		// answer with the feature that would add them.
		assert!(matches!(
			Target::from_str("browser"),
			Ok(Target::Dom) | Err(TestkitError::Feature { feature: "dom", .. })
		));
		assert!(matches!(
			Target::from_str("Simulator"),
			Ok(Target::Ios) | Err(TestkitError::Feature { feature: "ios", .. })
		));
		assert!(matches!(
			Target::from_str("EMULATOR"),
			Ok(Target::Android)
				| Err(TestkitError::Feature {
					feature: "android",
					..
				})
		));
		assert!(Target::from_str("nonsense").is_err());
	}

	#[test]
	fn every_built_target_has_a_name_that_reparses() -> Result<(), TestkitError> {
		for target in Target::built() {
			assert_eq!(Target::from_str(target.name())?, *target);
		}

		Ok::<(), TestkitError>(())
	}
}
