//! Runs the Rust samples in the canvist docs as doc tests.
//!
//! Each page with Rust samples is included below, so `cargo test --doc`
//! compiles and runs them against the current crates. `docs/check-snippets.ts`
//! fails when a page with Rust samples is missing from this list, and checks
//! the TypeScript and Dart samples.

// The pages are prose for readers, written for mdBook and the website, not
// API docs, so rustdoc's prose lints don't apply to them.
#![allow(
	clippy::doc_markdown,
	rustdoc::bare_urls,
	rustdoc::broken_intra_doc_links,
	rustdoc::invalid_html_tags
)]

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
	quick_start_desktop => "quick-start/desktop.md",
	concepts_extensions => "concepts/extensions.md",
	concepts_selections => "concepts/selections.md",
	extensions_build_your_own_editor => "extensions/build-your-own-editor.md",
	guides_import_export => "guides/import-export.md",
	guides_server_transforms => "guides/server-transforms.md",
	guides_theming => "guides/theming.md",
	guides_fonts => "guides/fonts.md",
	agents => "agents/readme.md",
	skill => "../../skills/canvist/SKILL.md",
}
