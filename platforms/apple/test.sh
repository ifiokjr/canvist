#!/usr/bin/env bash
# Run the `Canvist` package XCTests and the sample app UI test on an iOS
# Simulator (see `simulator.sh` for how it is chosen).
set -euo pipefail

apple_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$apple_dir/build.sh"

destination="platform=iOS Simulator,id=$("$apple_dir/simulator.sh")"
echo "Testing on: $destination"

cd "$apple_dir"
xcodebuild test \
	-scheme Canvist \
	-destination "$destination" \
	-derivedDataPath .build/xcode

cd "$apple_dir/Example"
xcodegen generate --quiet
rm -rf "$apple_dir/.build/example.xcresult"
xcodebuild test \
	-project CanvistExample.xcodeproj \
	-scheme CanvistExample \
	-destination "$destination" \
	-derivedDataPath "$apple_dir/.build/xcode-example" \
	-resultBundlePath "$apple_dir/.build/example.xcresult"
