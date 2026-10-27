#!/bin/sh
set -eu

XCRUN="${XCRUN:-xcrun}"
if ! command -v "$XCRUN" >/dev/null 2>&1; then
    echo "dx-apple-simctl: no xcrun executable $XCRUN on PATH; simulator operations need a macOS host with Xcode" >&2
    exit 127
fi
want="${DX_SIMCTL_DEVICE:-}"
if [ -z "$want" ]; then
    echo "dx-apple-simctl: DX_SIMCTL_DEVICE names the simulator by name or UDID" >&2
    exit 2
fi
if listing=$("$XCRUN" simctl list devices available 2>&1); then
    :
else
    status=$?
    echo "dx-apple-simctl: xcrun simctl cannot list simulators: $listing" >&2
    exit "$status"
fi
udids=$(printf '%s\n' "$listing" | awk -v want="$want" '
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
        if (name == want || udid == want) {
            print udid
        }
    }')
count=$(printf '%s\n' "$udids" | grep -c . || true)
if [ "$count" -eq 0 ]; then
    echo "dx-apple-simctl: no available simulator named '$want'; create one with xcrun simctl create" >&2
    exit 2
fi
if [ "$count" -gt 1 ]; then
    echo "dx-apple-simctl: '$want' names $count simulators; pass one UDID" >&2
    exit 2
fi
udid="$udids"

op="${DX_SIMCTL_OP:-}"
case "$op" in
    boot)
        exec "$XCRUN" simctl bootstatus "$udid" -b
        ;;
    install)
        app="${DX_SIMCTL_APP:-}"
        if [ -z "$app" ] || [ ! -e "$app" ]; then
            echo "dx-apple-simctl: DX_SIMCTL_APP names the built .ipa or .app" >&2
            exit 2
        fi
        case "$app" in
            *.app)
                exec "$XCRUN" simctl install "$udid" "$app"
                ;;
            *.ipa)
                work=$(mktemp -d)
                trap 'rm -rf "$work"' EXIT
                unzip -q "$app" -d "$work"
                for bundle in "$work"/Payload/*.app; do
                    if [ -d "$bundle" ]; then
                        "$XCRUN" simctl install "$udid" "$bundle"
                        exit 0
                    fi
                done
                echo "dx-apple-simctl: $app holds no Payload/*.app bundle" >&2
                exit 2
                ;;
            *)
                echo "dx-apple-simctl: $app is neither an .ipa nor an .app" >&2
                exit 2
                ;;
        esac
        ;;
    launch)
        bundle_id="${DX_SIMCTL_BUNDLE_ID:-}"
        if [ -z "$bundle_id" ]; then
            echo "dx-apple-simctl: DX_SIMCTL_BUNDLE_ID names the installed app" >&2
            exit 2
        fi
        exec "$XCRUN" simctl launch --terminate-running-process "$udid" "$bundle_id"
        ;;
    *)
        echo "dx-apple-simctl: unknown op '$op'" >&2
        exit 2
        ;;
esac
