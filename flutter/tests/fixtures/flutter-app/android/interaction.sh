#!/bin/sh
set -eu
serial="$1"
app_id="$2"
activity="$3"
shift 3
apk=""
for f in "$@"; do
    case "$f" in
    *_unsigned.apk) ;;
    *.apk) apk="$f" ;;
    esac
done
[ -n "$apk" ] || { echo "no signed apk in: $*"; exit 1; }
command -v adb >/dev/null || { echo "interaction: adb is not on PATH"; exit 1; }
adb -s "$serial" get-state >/dev/null 2>&1 || { echo "interaction: no emulator $serial"; exit 1; }
adb -s "$serial" install -r "$apk" >/dev/null
adb -s "$serial" shell am force-stop "$app_id"
started="$(adb -s "$serial" shell am start -W -n "$app_id/$activity" 2>&1)"
case "$started" in
*Error*) echo "$started"; exit 1 ;;
esac

dump() {
    adb -s "$serial" shell uiautomator dump /sdcard/dx_ui.xml >/dev/null 2>&1 || return 1
    adb -s "$serial" shell cat /sdcard/dx_ui.xml 2>/dev/null | tr -d '\r'
}

wait_for() {
    n=0
    while [ "$n" -lt 60 ]; do
        if dump | grep -q "$1"; then
            return 0
        fi
        sleep 1
        n=$((n + 1))
    done
    adb -s "$serial" logcat -d -t 80
    echo "interaction: ui never showed $1"
    return 1
}

wait_for 'No items'
bounds="$(dump | grep -o 'content-desc="Add item"[^>]*bounds="\[[0-9]*,[0-9]*\]\[[0-9]*,[0-9]*\]"' | grep -o '\[[0-9]*,[0-9]*\]\[[0-9]*,[0-9]*\]' | head -n 1)"
[ -n "$bounds" ] || { echo "interaction: no Add item button in the ui dump"; exit 1; }
set -- $(echo "$bounds" | tr '[],' '   ')
adb -s "$serial" shell input tap $((($1 + $3) / 2)) $((($2 + $4) / 2))
wait_for 'One item'
echo "interaction: tap on Add item changed the count to One item"
