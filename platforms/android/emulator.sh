#!/usr/bin/env bash
# Run a command with an Android device attached.
#
#   platforms/android/emulator.sh ./gradlew connectedDebugAndroidTest
#
# Uses an already-connected device when there is one. Otherwise boots the
# `CANVIST_AVD` AVD (default `canvist-api35`), creating it first if it does not
# exist, runs the command, and shuts the emulator down again. The command sees
# the device serial in `ANDROID_SERIAL`.
set -euo pipefail

export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
# Newer avdmanager releases default to an XDG directory the emulator does not
# search, so pin both tools to the same AVD home.
export ANDROID_AVD_HOME="${ANDROID_AVD_HOME:-$HOME/.android/avd}"
adb="$ANDROID_HOME/platform-tools/adb"
emulator="$ANDROID_HOME/emulator/emulator"
avdmanager="$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager"
sdkmanager="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
avd="${CANVIST_AVD:-canvist-api35}"
tmp="${TMPDIR:-/tmp}"
log="${tmp%/}/canvist-emulator.log"

attached_device() {
	"$adb" devices | awk 'NR > 1 && $2 == "device" { print $1; exit }'
}

if [ -z "$(attached_device)" ]; then
	if ! "$emulator" -list-avds | grep -qx "$avd"; then
		case "$(uname -m)" in
		arm64 | aarch64) abi="arm64-v8a" ;;
		*) abi="x86_64" ;;
		esac
		image="system-images;android-35;default;$abi"
		echo "Creating AVD $avd from $image" >&2
		# `yes` accepts the SDK licenses; it dies of SIGPIPE when sdkmanager exits,
		# so judge the install by sdkmanager's status alone.
		(set +o pipefail && yes | "$sdkmanager" --install "$image" >/dev/null)
		echo no | "$avdmanager" create avd --name "$avd" --package "$image" --device pixel_6 >&2
	fi

	echo "Booting $avd (log: $log)" >&2
	"$emulator" -avd "$avd" -no-window -no-audio -no-boot-anim -no-snapshot-save >"$log" 2>&1 &
	emulator_pid=$!
	trap '"$adb" emu kill >/dev/null 2>&1 || kill "$emulator_pid" 2>/dev/null || true' EXIT
	deadline=$((SECONDS + 300))
	until [ "$("$adb" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
		if ! kill -0 "$emulator_pid" 2>/dev/null; then
			echo "The emulator exited before booting; see $log" >&2
			exit 1
		fi
		if [ "$SECONDS" -ge "$deadline" ]; then
			echo "Timed out waiting for $avd to boot; see $log" >&2
			exit 1
		fi
		sleep 2
	done
fi

ANDROID_SERIAL="$(attached_device)"
export ANDROID_SERIAL
echo "Using Android device $ANDROID_SERIAL" >&2
"$@"
