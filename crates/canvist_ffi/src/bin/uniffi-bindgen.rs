//! The `uniffi-bindgen` CLI, built from this workspace so the generator always
//! matches the `uniffi` version the scaffolding was compiled with.
//!
//! ```sh
//! cargo run -p canvist_ffi --features bindgen --bin uniffi-bindgen -- \
//! 	generate --library target/debug/libcanvist_ffi.dylib --language swift --out-dir out
//! ```

fn main() {
	uniffi::uniffi_bindgen_main();
}
