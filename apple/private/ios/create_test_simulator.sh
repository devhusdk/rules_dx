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
if [ "${SIMULATOR_REUSE_SIMULATOR:-}" = 1 ]; then
    echo "dx-apple-test-simulator: test simulators are never reused; SIMULATOR_REUSE_SIMULATOR must be unset" >&2
    exit 2
fi
runtime="com.apple.CoreSimulator.SimRuntime.iOS-$(printf '%s' "$os" | tr . -)"
if listing=$("$XCRUN" simctl list runtimes available 2>&1); then
    :
else
    status=$?
    echo "dx-apple-test-simulator: xcrun simctl cannot list runtimes: $listing" >&2
    exit "$status"
fi
if ! printf '%s\n' "$listing" | awk -v want="$runtime" '$NF == want { found = 1 } END { exit !found }'; then
    echo "dx-apple-test-simulator: no available iOS $os simulator runtime $runtime; install it with xcodebuild -downloadPlatform iOS -buildVersion $os" >&2
    exit 2
fi
pgid=$(ps -o pgid= -p $$ 2>/dev/null | tr -d ' ') || pgid=""
case "$pgid" in
    "" | *[!0-9]*) pgid=$$ ;;
esac
# Sweep simulators left by runs whose process group is gone (timeouts,
# interrupts, failed deletes). Best effort; only exact dx owner names match.
if devices=$("$XCRUN" simctl list devices available 2>/dev/null); then
    printf '%s\n' "$devices" | awk '
        /^ +[^ =-]/ {
            line = $0
            sub(/^ +/, "", line)
            sub(/ +$/, "", line)
            if (!match(line, / \([0-9A-Fa-f-]+\) \([^()]*\)$/)) {
                next
            }
            name = substr(line, 1, RSTART - 1)
            rest = substr(line, RSTART + 2)
            udid = substr(rest, 1, index(rest, ")") - 1)
            if (name ~ /^dx .+ iOS [0-9.]+ pgid [0-9]+$/) {
                n = split(name, parts, " ")
                print udid, parts[n]
            }
        }' | while read -r old_udid old_pgid; do
        if kill -s 0 -- "-$old_pgid" 2>/dev/null; then
            continue
        fi
        "$XCRUN" simctl shutdown "$old_udid" </dev/null >/dev/null 2>&1 || true
        "$XCRUN" simctl delete "$old_udid" </dev/null >/dev/null 2>&1 || true
    done || true
fi
udid=$("$XCRUN" simctl create "dx $device iOS $os pgid $pgid" "$device" "$runtime")
"$XCRUN" simctl bootstatus "$udid" -b >&2 || {
    status=$?
    "$XCRUN" simctl delete "$udid" >&2 || true
    exit "$status"
}
printf '%s\n' "$udid"
