#!/bin/sh
set -eu
usage() {
    echo "dx-android-apk-set: $1" >&2
    exit 2
}
fail() {
    echo "dx-android-apk-set: $1" >&2
    exit 1
}
[ "$#" -eq 6 ] || usage "usage: build_apk_set.sh <bundletool> <aab> <device_spec.json> <keystore> <out.apks> <out_dir>"
bundletool="$1"
aab="$2"
spec="$3"
keystore="$4"
apks="$5"
out_dir="$6"
[ -f "$bundletool" ] || usage "no bundletool at $bundletool"
[ -f "$aab" ] || usage "no app bundle at $aab"
[ -f "$spec" ] || usage "no device spec at $spec"
[ -f "$keystore" ] || usage "no debug keystore at $keystore"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
reason() {
    sed -n 's/^\[BT:[^]]*\] Error: //p' "$1" | head -n 1
}
"$bundletool" --jvm_flag="-Djava.io.tmpdir=$work" build-apks --bundle="$aab" --device-spec="$spec" \
    --ks="$keystore" --ks-pass=pass:android --ks-key-alias=androiddebugkey --key-pass=pass:android \
    --overwrite --output="$apks" >"$work/log" 2>&1 || {
    grep -q "IncompatibleDeviceException" "$work/log" && fail "device spec $spec does not match $aab: $(reason "$work/log")"
    fail "bundletool build-apks rejects $aab: $(reason "$work/log")"
}
mkdir -p "$out_dir"
"$bundletool" --jvm_flag="-Djava.io.tmpdir=$work" extract-apks --apks="$apks" --device-spec="$spec" \
    --output-dir="$out_dir" >"$work/log" 2>&1 || {
    grep -q "IncompatibleDeviceException" "$work/log" && fail "device spec $spec does not match $aab: $(reason "$work/log")"
    fail "bundletool extract-apks rejects $apks: $(reason "$work/log")"
}
set -- "$out_dir"/*.apk
[ -f "$1" ] || fail "device spec $spec selects no APK from $aab"
