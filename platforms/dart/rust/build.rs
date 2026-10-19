//! `flutter_rust_bridge` macros expand to `#[cfg(frb_expand)]` checks; declare
//! the cfg so rustc does not warn about it.

fn main() {
	println!("cargo::rustc-check-cfg=cfg(frb_expand)");
}
