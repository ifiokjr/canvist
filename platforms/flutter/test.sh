#!/usr/bin/env bash
# Run the example app's device tests on an iOS Simulator and an Android
# emulator: the integration tests, then the Patrol suites, which drive the app
# through the device itself (the platform keyboard, native taps and swipes,
# system dark mode) and compare its pixels with server rendering.
#
# `CANVIST_FLUTTER_PLATFORMS` picks the platforms (default: `ios android` on
# macOS, `android` elsewhere). Uses fvm when it is installed so the Flutter
# version comes from `.fvmrc`. Patrol's CLI must be on the PATH:
# `dart pub global activate patrol_cli 4.8.0`.
#
# `test.sh --on-device <id>` runs both suites on one booted device.
set -euo pipefail

flutter_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
platforms_dir="$(dirname "$flutter_dir")"

if command -v fvm >/dev/null; then
	flutter=(fvm flutter)
else
	flutter=(flutter)
fi

# One entrypoint means one build, install and launch per device. Each extra
# `_test.dart` file relaunches the app, and on CI simulators a relaunch can
# hang `simctl` until the job times out.
entrypoint=integration_test/canvist_test.dart
# `CANVIST_PERF_SLACK` multiplies the keystroke-to-frame budget, for slow
# emulators.
define="--dart-define=CANVIST_PERF_SLACK=${CANVIST_PERF_SLACK:-1}"

if [ "${1:-}" = "--on-device" ]; then
	device="$2"
	cd "$flutter_dir/example"
	# A busy emulator raises "isn't responding" dialogs over the app, which
	# swallow the native taps and swipes Patrol sends. Hide them.
	if command -v adb >/dev/null && adb devices | grep -q "^${device}[[:space:]]"; then
		adb -s "$device" shell settings put global hide_error_dialogs 1
	fi
	"${flutter[@]}" test "$entrypoint" "$define" -d "$device"
	patrol test --flutter-command "${flutter[*]}" --target patrol_test -d "$device"
	exit
fi

if ! command -v patrol >/dev/null; then
	echo "Patrol's CLI is not on the PATH; install it with: dart pub global activate patrol_cli 4.8.0" >&2
	exit 1
fi

if [ "$(uname)" = "Darwin" ]; then
	default_platforms="ios android"
else
	default_platforms="android"
fi

(cd "$flutter_dir/example" && "${flutter[@]}" pub get)

for platform in ${CANVIST_FLUTTER_PLATFORMS:-$default_platforms}; do
	case "$platform" in
	ios)
		"$0" --on-device "$("$platforms_dir/apple/simulator.sh")"
		;;
	android)
		# shellcheck disable=SC2016 # Expanded by the inner shell.
		"$platforms_dir/android/emulator.sh" bash -c '"$0" --on-device "$ANDROID_SERIAL"' "$0"
		;;
	*)
		echo "Unknown platform '$platform' (expected ios or android)" >&2
		exit 1
		;;
	esac
done
