#!/usr/bin/env bash
# Cross-compile `libcanvist_ffi.so` for Android (arm64-v8a, x86_64) with
# cargo-ndk and generate the UniFFI Kotlin sources for the `canvist` module.
set -euo pipefail

android_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$android_dir/../.." && pwd)"
module="$android_dir/canvist/src/main"
targets=(aarch64-linux-android x86_64-linux-android)

export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
	ndk="$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1)"
	if [ -z "$ndk" ]; then
		echo "No NDK under $ANDROID_HOME/ndk. Install one with: sdkmanager 'ndk;29.0.13599879'" >&2
		exit 1
	fi
	export ANDROID_NDK_HOME="$ndk"
fi

cd "$root"
rustup target add "${targets[@]}" >/dev/null

rm -rf "$module/jniLibs"
cargo ndk \
	--target arm64-v8a \
	--target x86_64 \
	--platform 24 \
	--output-dir "$module/jniLibs" \
	build --locked --release --package canvist_ffi --lib

rm -rf "$module/kotlin/io/github/ifiokjr/canvist/canvist_ffi.kt"
cargo run --locked --quiet --package canvist_ffi --features bindgen --bin uniffi-bindgen -- \
	generate \
	--library "$module/jniLibs/arm64-v8a/libcanvist_ffi.so" \
	--language kotlin \
	--no-format \
	--out-dir "$module/kotlin"

echo "Built $(find "$module/jniLibs" -name '*.so' | sort | tr '\n' ' ')"
