#!/bin/sh
set -eu

XCRUN="${XCRUN:-xcrun}"
if ! command -v "$XCRUN" >/dev/null 2>&1; then
    echo "dx-apple-test-simulator: no xcrun executable $XCRUN on PATH; simulator tests need a macOS host with Xcode" >&2
    exit 127
fi
device="${SIMULATOR_DEVICE_TYPE:-}"
os="${SIMULATOR_OS_VERSION:-}"
if [ -z "$device" ] || [ -z "$os" ]; then
    echo "dx-apple-test-simulator: SIMULATOR_DEVICE_TYPE and SIMULATOR_OS_VERSION name the simulator to create" >&2
    exit 2
fi
runtime="com.apple.CoreSimulator.SimRuntime.iOS-$(printf '%s' "$os" | tr . -)"
if ! listing=$("$XCRUN" simctl list runtimes available 2>&1); then
    echo "dx-apple-test-simulator: xcrun simctl cannot list runtimes: $listing" >&2
    exit 127
fi
if ! printf '%s\n' "$listing" | awk -v want="$runtime" '$NF == want { found = 1 } END { exit !found }'; then
    echo "dx-apple-test-simulator: no available iOS $os simulator runtime $runtime; install it with xcodebuild -downloadPlatform iOS -buildVersion $os" >&2
    exit 2
fi
udid=$("$XCRUN" simctl create "dx $device iOS $os ${XCTESTRUN_RUNNER_PID:-$$}" "$device" "$runtime")
"$XCRUN" simctl bootstatus "$udid" -b >&2 || {
    status=$?
    "$XCRUN" simctl delete "$udid" >&2 || true
    exit "$status"
}
printf '%s\n' "$udid"
