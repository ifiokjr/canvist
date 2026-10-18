#!/usr/bin/env bash
# Run the example app's integration tests on an iOS Simulator and an Android
# emulator. `CANVIST_FLUTTER_PLATFORMS` picks the platforms (default: `ios
# android` on macOS, `android` elsewhere). Uses fvm when it is installed so the
# Flutter version comes from `.fvmrc`.
set -euo pipefail

flutter_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
platforms_dir="$(dirname "$flutter_dir")"
if [ "$(uname)" = "Darwin" ]; then
	default_platforms="ios android"
else
	default_platforms="android"
fi

if command -v fvm >/dev/null; then
	flutter=(fvm flutter)
else
	flutter=(flutter)
fi

cd "$flutter_dir/example"
"${flutter[@]}" pub get

for platform in ${CANVIST_FLUTTER_PLATFORMS:-$default_platforms}; do
	case "$platform" in
	ios)
		"${flutter[@]}" test integration_test -d "$("$platforms_dir/apple/simulator.sh")"
		;;
	android)
		# shellcheck disable=SC2016 # Expanded by the inner shell.
		"$platforms_dir/android/emulator.sh" bash -c '"$@" test integration_test -d "$ANDROID_SERIAL"' _ "${flutter[@]}"
		;;
	*)
		echo "Unknown platform '$platform' (expected ios or android)" >&2
		exit 1
		;;
	esac
done
