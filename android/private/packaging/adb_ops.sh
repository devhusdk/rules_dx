#!/bin/sh
set -eu
ADB="${ADB:-adb}"
command -v "$ADB" >/dev/null 2>&1 || { echo "dx-android-adb: no adb executable $ADB on PATH" >&2; exit 127; }
serial="${DX_ADB_SERIAL:-}"
if [ -z "$serial" ]; then
    echo "dx-android-adb: DX_ADB_SERIAL names the target device or emulator" >&2
    exit 2
fi
op="${DX_ADB_OP:-}"
case "$op" in
    install)
        apk="${DX_ADB_APK:-}"
        if [ -z "$apk" ]; then
            echo "dx-android-adb: DX_ADB_APK names the APK file" >&2
            exit 2
        fi
        exec "$ADB" -s "$serial" install -r "$apk"
        ;;
    install-multiple)
        dir="${DX_ADB_APKS:-}"
        if [ -z "$dir" ] || [ ! -d "$dir" ]; then
            echo "dx-android-adb: DX_ADB_APKS names the APK set directory" >&2
            exit 2
        fi
        set -- "$dir"/*.apk
        if [ ! -f "$1" ]; then
            echo "dx-android-adb: $dir holds no APK" >&2
            exit 2
        fi
        exec "$ADB" -s "$serial" install-multiple -r "$@"
        ;;
    start)
        app="${DX_ADB_APP:-}"
        activity="${DX_ADB_ACTIVITY:-}"
        if [ -z "$app" ] || [ -z "$activity" ]; then
            echo "dx-android-adb: DX_ADB_APP and DX_ADB_ACTIVITY name the component" >&2
            exit 2
        fi
        exec "$ADB" -s "$serial" shell am start -n "$app/$activity"
        ;;
    log)
        exec "$ADB" -s "$serial" logcat -d
        ;;
    *)
        echo "dx-android-adb: unknown op '$op'" >&2
        exit 2
        ;;
esac
