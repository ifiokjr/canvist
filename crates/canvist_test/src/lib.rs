//! # canvist_test
//!
//! Integration test utilities for the canvist canvas editor.
//!
//! This crate provides helpers for running Playwright-based browser tests
//! against the WASM build of canvist. Tests in `tests/` use these helpers to
//! automate a real browser and verify that the editor renders and behaves
//! correctly.
//!
//! ## Running tests
//!
//! ```bash
//! # Build the WASM package first.
//! build:wasm
//!
//! # Run the Playwright tests.
//! test:playwright
//! ```

use std::path::Path;
use std::path::PathBuf;

/// Return the path to the WASM package directory.
///
/// This assumes the workspace root layout and that `build:wasm` has been run.
#[must_use]
pub fn wasm_pkg_dir() -> PathBuf {
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	manifest_dir
		.parent() // crates/
		.and_then(|p| p.parent()) // workspace root
		.map(|root| root.join("crates/canvist_wasm/pkg"))
		.unwrap_or_else(|| PathBuf::from("crates/canvist_wasm/pkg"))
}

/// Return the path to the workspace root.
#[must_use]
pub fn workspace_root() -> PathBuf {
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	manifest_dir
		.parent()
		.and_then(|p| p.parent())
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from("."))
}
