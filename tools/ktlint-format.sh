#!/usr/bin/env bash
# dprint's Kotlin formatter (called from the `exec` plugin with the file path).
#
# ktlint 1.8.0 crashes with `UnknownFormatConversionException` when it
# autocorrects a line that contains a `%` specifier — the code text leaks into
# a `String.format` call while rendering the violation. When that happens the
# run exits non-zero with empty stdout, so fall back to the unformatted input:
# dprint keeps a valid file, and CI's `lint:kotlin` step still reports the
# violations ktlint could not autocorrect.
set -uo pipefail

input="$(mktemp "${TMPDIR:-/tmp}/canvist-ktlint.XXXXXX.kt")"
output="$(mktemp "${TMPDIR:-/tmp}/canvist-ktlint.XXXXXX.kt")"
trap 'rm -f "$input" "$output"' EXIT
cat >"$input"

if ktlint --stdin -F --stdin-path "$1" --log-level=none --ignore-autocorrect-failures \
	<"$input" >"$output" 2>/dev/null && [ -s "$output" ]; then
	cat "$output"
else
	echo "ktlint could not format $1; falling back to the unformatted file" >&2
	cat "$input"
fi
