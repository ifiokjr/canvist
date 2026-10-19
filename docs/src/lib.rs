//! Runs the Rust samples in the canvist docs as doc tests.
//!
//! Each page with Rust samples is included below, so `cargo test --doc`
//! compiles and runs them against the current crates. `docs/check-snippets.ts`
//! fails when a page with Rust samples is missing from this list, and checks
//! the TypeScript and Dart samples.

macro_rules! pages {
	($($name:ident => $path:literal),* $(,)?) => {
		$(
			#[doc = include_str!($path)]
			pub mod $name {}
		)*
	};
}

pages! {
	quick_start_server_rust => "quick-start/server-rust.md",
	quick_start_ssr => "quick-start/ssr.md",
	concepts_extensions => "concepts/extensions.md",
	concepts_selections => "concepts/selections.md",
	extensions_build_your_own_editor => "extensions/build-your-own-editor.md",
}
