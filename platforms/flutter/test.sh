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
# `test.sh --on-device <id>` runs both suites on one booted device. A launch
# that stalls waiting for the app's VM service is killed and retried (three
# attempts); `CANVIST_LAUNCH_STALL_SECONDS` tunes how long a wait counts as a
# stall (default: 300).
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
	mkdir -p build
	is_android=0
	if command -v adb >/dev/null && adb devices | grep -q "^${device}[[:space:]]"; then
		is_android=1
		# A busy emulator raises "isn't responding" dialogs over the app,
		# which swallow the native taps and swipes Patrol sends. Hide them.
		adb -s "$device" shell settings put global hide_error_dialogs 1
		# The device's log is the only record of a crash once the emulator
		# stops: keep it for the job to upload.
		adb -s "$device" logcat -c
		adb -s "$device" logcat -v time >build/logcat.txt 2>&1 &
		logcat=$!
		trap 'kill "$logcat" 2>/dev/null' EXIT
	fi
	# Flutter installs, launches, and then waits for the app's "Dart VM
	# service is listening" announcement with no timeout of its own; on a
	# busy device the log stream it scrapes can miss the line, and the wait
	# outlives the job even though the app is running. Keep the verbose log,
	# which names the last command Flutter ran, show progress from it, and
	# let `run_tests` below catch the stalled launch.
	log="build/flutter-test.log"
	progress='^([0-9]+:[0-9]{2} [+~-]|PERF )|Xcode build done|executing: .*simctl (install|launch) |Waiting for VM Service|VM Service URL on device'
	# A working launch announces the service in seconds, while Xcode and
	# Gradle builds also go silent for minutes; once Flutter has installed or
	# launched the app, silence this long means the launch hung (a wedged
	# `simctl` call, or the log stream missing the announcement).
	stall_seconds="${CANVIST_LAUNCH_STALL_SECONDS:-300}"
	# The app's ids, to stop a stalled launch's app before retrying it.
	ios_bundle_id=io.github.ifiokjr.canvist.canvistFlutterExample
	android_application_id=io.github.ifiokjr.canvist.canvist_flutter_example

	# Run `flutter test`, echoing progress lines as they appear. Returns
	# Flutter's exit status — or 1 with `stalled=1` once the launch spent
	# `stall_seconds` silent past its install or launch commands without a
	# VM service announcement: Flutter then dies with the log stream it
	# spawned, and any app it launched (which is running fine; Flutter just
	# never heard it) is stopped, so the caller can retry the launch with a
	# fresh log stream.
	run_tests() {
		stalled=0
		attempt_log="build/flutter-test.attempt$1.log"
		rm -f "$attempt_log"
		# `set -m` puts Flutter in its own process group, so the kill below
		# reaches the whole tool tree.
		set -m
		"${flutter[@]}" test "$entrypoint" "$define" -d "$device" --verbose >"$attempt_log" 2>&1 &
		flutter_pid=$!
		set +m
		printed=0
		quiet_since=0
		while kill -0 "$flutter_pid" 2>/dev/null; do
			sleep 20
			lines=0
			if [ -f "$attempt_log" ]; then
				lines=$(wc -l <"$attempt_log" | tr -d '[:space:]')
			fi
			if [ "$lines" -gt "$printed" ]; then
				tail -n "+$((printed + 1))" "$attempt_log" | grep -E "$progress" || true
				printed=$lines
				quiet_since=0
				continue
			fi
			if [ "$quiet_since" -eq 0 ]; then
				quiet_since=$(date +%s)
			fi
			# A quiet build never reached the launch commands, and a launch
			# that announced its VM service may run quiet tests; neither
			# stalls. Only silence with launch commands behind it and no
			# announcement yet does.
			launched=$(grep -cE 'Waiting for VM Service|executing: .*simctl (install|launch) ' "$attempt_log" || true)
			launched=${launched:-0}
			urls=$(grep -c 'VM Service URL on device' "$attempt_log" || true)
			urls=${urls:-0}
			if [ "$launched" -gt 0 ] && [ "$urls" -eq 0 ] && [ "$(($(date +%s) - quiet_since))" -ge "$stall_seconds" ]; then
				stalled=1
				echo "Launch stalled: Flutter went silent for $((stall_seconds / 60)) minutes without the app's VM service announcement." >&2
				kill -- "-$flutter_pid" 2>/dev/null || true
				sleep 5
				kill -KILL -- "-$flutter_pid" 2>/dev/null || true
				if [ "$is_android" -eq 1 ]; then
					adb -s "$device" shell am force-stop "$android_application_id" 2>/dev/null || true
				else
					xcrun simctl terminate "$device" "$ios_bundle_id" 2>/dev/null || true
				fi
				wait "$flutter_pid" 2>/dev/null || true
				return 1
			fi
		done
		lines=0
		if [ -f "$attempt_log" ]; then
			lines=$(wc -l <"$attempt_log" | tr -d '[:space:]')
		fi
		if [ "$lines" -gt "$printed" ]; then
			tail -n "+$((printed + 1))" "$attempt_log" | grep -E "$progress" || true
		fi
		wait "$flutter_pid"
	}

	for attempt in 1 2 3; do
		if run_tests "$attempt"; then
			cat "$attempt_log" >>"$log"
			rm -f "$attempt_log"
			break
		fi
		cat "$attempt_log" >>"$log" 2>/dev/null || true
		rm -f "$attempt_log"
		if [ "$stalled" -ne 1 ] || [ "$attempt" -eq 3 ]; then
			tail -n 200 "$log"
			exit 1
		fi
		echo "Retrying the launch (attempt $((attempt + 1)) of 3)." >&2
	done

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
