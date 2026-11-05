#!/bin/sh
set -eu
mode="$1"
checker="$2"
data="$(dirname "$3")"
app="org.example.custom.dxrustapk"
expect() {
    want="$1"
    shift
    set +e
    sh "$checker" "$@" >/dev/null 2>"$err"
    code="$?"
    set -e
    if [ "$code" != "$want" ]; then
        echo "device_check $* exits $code, want $want" >&2
        cat "$err" >&2
        exit 1
    fi
}
err="$(mktemp)"
case "$mode" in
    ok)
        expect 0 logcat "$data/logcat_ok.txt" 30
        expect 0 package "$data/dumpsys_ok.txt" "$app" 7 33 x86_64 android.permission.VIBRATE android.permission.ACCESS_NETWORK_STATE
        ;;
    rejects)
        expect 1 logcat "$data/logcat_no_input.txt" 30
        expect 1 logcat "$data/logcat_out_of_order.txt" 30
        expect 1 logcat "$data/logcat_asset_mismatch.txt" 30
        expect 1 logcat "$data/logcat_ok.txt" 31
        expect 1 package "$data/dumpsys_wrong_abi.txt" "$app" 7 33 x86_64 android.permission.VIBRATE android.permission.ACCESS_NETWORK_STATE
        expect 1 package "$data/dumpsys_ok.txt" "$app" 8 33 x86_64 android.permission.VIBRATE android.permission.ACCESS_NETWORK_STATE
        expect 1 package "$data/dumpsys_ok.txt" "$app" 7 33 x86_64 android.permission.INTERNET
        expect 1 package "$data/dumpsys_ok.txt" com.example.dxrustapk 7 33 x86_64
        expect 2
        expect 2 logcat
        expect 2 inspect "$data/logcat_ok.txt"
        ;;
    *)
        echo "unknown mode $mode" >&2
        exit 2
        ;;
esac
