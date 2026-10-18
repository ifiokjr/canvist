#!/usr/bin/env bash
# Build `CanvistFFI.xcframework` (iOS device + simulator, arm64) and generate
# the Swift bindings that the `Canvist` Swift package compiles.
set -euo pipefail

apple_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$apple_dir/../.." && pwd)"
targets=(aarch64-apple-ios aarch64-apple-ios-sim)
target_dir="$root/target"
build_dir="$apple_dir/.build/canvist-ffi"
xcframework="$apple_dir/Frameworks/CanvistFFI.xcframework"
generated="$apple_dir/Sources/Canvist/Canvist.swift"

cd "$root"
rustup target add "${targets[@]}" >/dev/null

for target in "${targets[@]}"; do
	cargo build --locked --release --package canvist_ffi --lib --target "$target"
done

rm -rf "$build_dir"
mkdir -p "$build_dir/bindings"
cargo run --locked --quiet --package canvist_ffi --features bindgen --bin uniffi-bindgen -- \
	generate \
	--library "$target_dir/aarch64-apple-ios-sim/release/libcanvist_ffi.a" \
	--language swift \
	--no-format \
	--out-dir "$build_dir/bindings"

# Nest the headers in a directory named after the module so the module map
# cannot collide with other XCFrameworks in the same app.
headers="$build_dir/headers/CanvistFFI"
mkdir -p "$headers"
cp "$build_dir/bindings/CanvistFFI.h" "$headers/"
cp "$build_dir/bindings/CanvistFFI.modulemap" "$headers/module.modulemap"

rm -rf "$xcframework"
library_args=()
for target in "${targets[@]}"; do
	library_args+=(-library "$target_dir/$target/release/libcanvist_ffi.a" -headers "$build_dir/headers")
done
xcodebuild -create-xcframework "${library_args[@]}" -output "$xcframework" >/dev/null

mkdir -p "$(dirname "$generated")"
cp "$build_dir/bindings/Canvist.swift" "$generated"
echo "Built $xcframework"
echo "Generated $generated"
