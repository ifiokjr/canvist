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

# One entrypoint means one build, install and launch per device. Each extra
# `_test.dart` file relaunches the app, and on CI simulators a relaunch can
# hang `simctl` until the job times out.
entrypoint=integration_test/canvist_test.dart
# `CANVIST_PERF_SLACK` multiplies the keystroke-to-frame budget, for slow
# emulators.
define="--dart-define=CANVIST_PERF_SLACK=${CANVIST_PERF_SLACK:-1}"

for platform in ${CANVIST_FLUTTER_PLATFORMS:-$default_platforms}; do
	case "$platform" in
	ios)
		"${flutter[@]}" test "$entrypoint" "$define" -d "$("$platforms_dir/apple/simulator.sh")"
		;;
	android)
		# shellcheck disable=SC2016 # Expanded by the inner shell.
		"$platforms_dir/android/emulator.sh" bash -c 'entrypoint=$1; define=$2; shift 2; "$@" test "$entrypoint" "$define" -d "$ANDROID_SERIAL"' _ "$entrypoint" "$define" "${flutter[@]}"
		;;
	*)
		echo "Unknown platform '$platform' (expected ios or android)" >&2
		exit 1
		;;
	esac
done
