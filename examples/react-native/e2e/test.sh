#!/usr/bin/env bash
# Build the example app in Release and run the Maestro flows in this folder
# on an iOS Simulator or an Android emulator:
#
#   examples/react-native/e2e/test.sh ios
#   examples/react-native/e2e/test.sh android
#
# Release builds bundle the JavaScript, so no Metro server is needed, and the
# benchmark flow measures what people run. It first builds canvist's engine
# for the platform (`platforms/apple/build.sh`, or `platforms/android/build.sh`
# and a Maven Local publication of `canvist-android`); set
# `CANVIST_SKIP_ENGINE_BUILD=1` to reuse the last one, and `CANVIST_PERF_SLACK`
# to multiply the benchmark's budgets on a slow device. `@canvist/headless`
# must be built (`npm run build` in `packages/canvist-headless`) and this
# example's dependencies installed.
set -euo pipefail

platform="${1:?usage: test.sh ios|android}"
e2e_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
example="$(cd "$e2e_dir/.." && pwd)"
root="$(cd "$example/../.." && pwd)"
results="$example/build/e2e-$platform"

export MAESTRO_CLI_NO_ANALYTICS=1
export MAESTRO_CLI_ANALYSIS_NOTIFICATION_DISABLED=true

case "$platform" in
ios)
	if [ -z "${CANVIST_SKIP_ENGINE_BUILD:-}" ]; then
		"$root/platforms/apple/build.sh"
	fi

	udid="$("$root/platforms/apple/simulator.sh")"
	echo "Testing on simulator $udid"

	cd "$example/ios"
	# CocoaPods fails on non-UTF-8 locales.
	LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8 pod install
	xcodebuild build -quiet \
		-workspace CanvistExample.xcworkspace \
		-scheme CanvistExample \
		-configuration Release \
		-sdk iphonesimulator \
		-destination "id=$udid" \
		-derivedDataPath "$example/build/ios"

	xcrun simctl install "$udid" "$example/build/ios/Build/Products/Release-iphonesimulator/CanvistExample.app"
	maestro --device "$udid" test "$e2e_dir" --test-output-dir "$results" -e PERF_SLACK="${CANVIST_PERF_SLACK:-1}"
	;;
android)
	export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
	studio_jdk="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
	if [ -z "${JAVA_HOME:-}" ] && [ -d "$studio_jdk" ]; then
		export JAVA_HOME="$studio_jdk"
	fi

	if [ -z "${CANVIST_SKIP_ENGINE_BUILD:-}" ]; then
		"$root/platforms/android/build.sh"
		(cd "$root/platforms/android" && ./gradlew :canvist:publishToMavenLocal --console=plain)
	fi

	adb="$ANDROID_HOME/platform-tools/adb"
	# shellcheck disable=SC2016 # The inner shell expands these.
	run_flows='
		set -euo pipefail
		adb="$ANDROID_HOME/platform-tools/adb"
		abi="$("$adb" -s "$ANDROID_SERIAL" shell getprop ro.product.cpu.abi | tr -d "\r")"
		cd "$CANVIST_EXAMPLE/android"
		./gradlew assembleRelease --console=plain -PreactNativeArchitectures="$abi"
		"$adb" -s "$ANDROID_SERIAL" install -r app/build/outputs/apk/release/app-release.apk
		maestro --device "$ANDROID_SERIAL" test "$CANVIST_E2E" --test-output-dir "$CANVIST_RESULTS" -e PERF_SLACK="${CANVIST_PERF_SLACK:-1}"
	'
	export CANVIST_EXAMPLE="$example" CANVIST_E2E="$e2e_dir" CANVIST_RESULTS="$results"

	# The flows install the app, clear its data, and type into it, so they run
	# on the device `ANDROID_SERIAL` names or an emulator, never on a phone
	# that happens to be plugged in.
	if [ -z "${ANDROID_SERIAL:-}" ]; then
		ANDROID_SERIAL="$("$adb" devices | awk 'NR > 1 && $2 == "device" && $1 ~ /^emulator-/ { print $1; exit }')"
	fi

	if [ -n "$ANDROID_SERIAL" ]; then
		export ANDROID_SERIAL
		echo "Testing on $ANDROID_SERIAL"
		bash -c "$run_flows"
	elif [ -n "$("$adb" devices | awk 'NR > 1 && $2 == "device"')" ]; then
		echo "Only physical devices are attached; set ANDROID_SERIAL to test on one." >&2
		exit 1
	else
		# Boots an emulator, which sets ANDROID_SERIAL.
		"$root/platforms/android/emulator.sh" bash -c "$run_flows"
	fi
	;;
*)
	echo "usage: test.sh ios|android" >&2
	exit 2
	;;
esac
