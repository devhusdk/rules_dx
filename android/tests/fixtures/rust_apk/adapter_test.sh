#!/bin/sh
set -eu
op="$1"
launcher="$2"
fake_adb="$3"
serial="emulator-5554"
export ADB="$fake_adb"
export ADB_CAPTURE="$(mktemp)"
export DX_ADB_SERIAL="$serial"
export DX_ADB_OP="$op"
if [ "$op" = "install" ]; then
    export DX_ADB_APK="$4"
elif [ "$op" = "start" ]; then
    export DX_ADB_APP="com.example.dxrustapk"
    export DX_ADB_ACTIVITY="android.app.NativeActivity"
fi
if [ "$op" = "missing" ]; then
    out="$(mktemp)"
    set +e
    ADB="/nonexistent/adb" "$launcher" 2>"$out"
    code="$?"
    set -e
    if [ "$code" != "127" ]; then
        echo "missing adb exits $code, want 127" >&2
        exit 1
    fi
    grep -q "dx-android-adb: no adb executable" "$out" || { echo "missing adb names the fix"; cat "$out"; exit 1; }
    exit 0
fi
if [ "${ADB_EXIT:-0}" != "0" ]; then
    set +e
    "$launcher" 2>/dev/null
    code="$?"
    set -e
    if [ "$code" = "0" ]; then
        echo "adapter hid adb failure" >&2
        exit 1
    fi
    if [ "$code" != "$ADB_EXIT" ]; then
        echo "adapter exits $code, want $ADB_EXIT" >&2
        exit 1
    fi
    exit 0
fi
"$launcher"
captured="$(cat "$ADB_CAPTURE")"
case "$op" in
    install)
        prefix="-s $serial install -r "
        case "$captured" in
            "$prefix"*)
                apk="${captured#$prefix}"
                [ -f "$apk" ] || { echo "install names no apk file: $apk" >&2; exit 1; }
                ;;
            *)
                echo "install argv '$captured' misses '$prefix'" >&2
                exit 1
                ;;
        esac
        ;;
    start)
        want="-s $serial shell am start -n com.example.dxrustapk/android.app.NativeActivity"
        [ "$captured" = "$want" ] || { echo "start argv '$captured' differs from '$want'" >&2; exit 1; }
        ;;
    log)
        want="-s $serial logcat -d"
        [ "$captured" = "$want" ] || { echo "log argv '$captured' differs from '$want'" >&2; exit 1; }
        ;;
    *)
        echo "unknown adapter op '$op'" >&2
        exit 1
        ;;
esac
