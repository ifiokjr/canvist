#!/usr/bin/env bash
# dprint's Swift formatter (called from the `exec` plugin with no arguments;
# stdin carries the file and the formatted result is printed to stdout).
#
# nixpkgs still packages swift-format 5.10.1, whose parser predates Swift 6
# syntax such as typed throws, so it fails to parse a few canvist files. Fall
# back to the unformatted input when that happens: dprint keeps a valid file,
# and `lint:swift` (SwiftLint plus the Xcode toolchain's newer `swift format`
# on macOS) still covers what this formatter cannot parse.
set -uo pipefail

input="$(mktemp "${TMPDIR:-/tmp}/canvist-swift-format.XXXXXX.swift")"
output="$(mktemp "${TMPDIR:-/tmp}/canvist-swift-format.XXXXXX.swift")"
trap 'rm -f "$input" "$output"' EXIT
cat >"$input"

if swift-format format <"$input" >"$output" 2>/dev/null && [ -s "$output" ]; then
	cat "$output"
else
	echo "swift-format could not format a Swift 6 file; falling back to the unformatted file" >&2
	cat "$input"
fi
