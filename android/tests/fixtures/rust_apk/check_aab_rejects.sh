#!/bin/sh
set -eu
check="$1"
bundletool="$2"
apk="$3"
unsigned="$4"
malformed="$TEST_TMPDIR/malformed.aab"
cp "$apk" "$malformed"
expect() {
    label="$1"
    want="$2"
    pattern="$3"
    shift 3
    set +e
    out="$("$check" "$@" 2>&1)"
    code="$?"
    set -e
    [ "$code" = "$want" ] || { echo "$label exited $code, want $want: $out"; exit 1; }
    echo "$out" | grep -q "^dx-android-check: $pattern" || { echo "$label misses '$pattern': $out"; exit 1; }
}
expect "malformed bundle" 1 "bundletool validate rejects" "$bundletool" "$malformed"
expect "abi on apk" 2 "--abi and --resource apply to App Bundles only" "$bundletool" "$unsigned" --abi arm64-v8a
expect "missing bundle" 2 "no package at" "$bundletool" "$apk.missing"
expect "missing bundletool" 2 "no bundletool at" "$bundletool.missing" "$apk"
expect "unknown argument" 2 "unknown argument '--sign'" "$bundletool" "$apk" --sign
expect "abi without value" 2 "--abi needs an ABI name" "$bundletool" "$apk" --abi
expect "no arguments" 2 "usage:" "$bundletool"
