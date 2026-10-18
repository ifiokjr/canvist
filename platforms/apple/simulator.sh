#!/usr/bin/env bash
# Print the UDID of the iOS Simulator to test on, booting it first.
# Set `CANVIST_IOS_SIMULATOR` to a simulator name or UDID to choose one;
# otherwise the first available iPhone is used.
set -euo pipefail

choice="${CANVIST_IOS_SIMULATOR:-iPhone}"
udid="$(xcrun simctl list devices available "$choice" | grep -Eo '[0-9A-F]{8}(-[0-9A-F]{4}){3}-[0-9A-F]{12}' | head -n 1)"
if [ -z "$udid" ]; then
	echo "No available simulator matches '$choice'. Install one with: xcodebuild -downloadPlatform iOS" >&2
	exit 1
fi

# Booting an already-booted device fails; `bootstatus` catches real failures.
xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b >/dev/null
echo "$udid"
