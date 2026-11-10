#!/bin/sh
set -eu

XCRUN="${XCRUN:-xcrun}"
udid="${SIMULATOR_UDID:-}"
if [ -z "$udid" ]; then
    echo "dx-apple-test-simulator: SIMULATOR_UDID names the simulator to delete" >&2
    exit 2
fi
"$XCRUN" simctl shutdown "$udid" >/dev/null 2>&1 || true
if ! "$XCRUN" simctl delete "$udid"; then
    echo "dx-apple-test-simulator: warning: could not delete simulator $udid; the next test run on this host sweeps it" >&2
fi
exit 0
