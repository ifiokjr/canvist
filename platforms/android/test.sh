#!/usr/bin/env bash
# Run the instrumented tests of the `canvist` library and the sample app on an
# Android device or emulator (see `emulator.sh` for how one is chosen).
set -euo pipefail

android_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$android_dir/build.sh"

# Gradle needs JDK 17+; default to the one bundled with Android Studio.
studio_jdk="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
if [ -z "${JAVA_HOME:-}" ] && [ -d "$studio_jdk" ]; then
	export JAVA_HOME="$studio_jdk"
fi

cd "$android_dir"
"$android_dir/emulator.sh" ./gradlew connectedDebugAndroidTest --console=plain
