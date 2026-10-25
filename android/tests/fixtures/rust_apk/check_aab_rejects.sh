#!/bin/sh
set -eu
check="$1"
bundletool="$2"
malformed="$3"
apk="$4"
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
    echo "$out" | grep -q "^dx-android-check-aab: $pattern" || { echo "$label misses '$pattern': $out"; exit 1; }
}
expect "malformed bundle" 1 "bundletool validate rejects" "$bundletool" "$malformed"
expect "apk as bundle" 1 "bundletool validate rejects" "$bundletool" "$apk"
expect "missing bundle" 2 "no app bundle at" "$bundletool" "$apk.missing"
expect "missing bundletool" 2 "no bundletool at" "$bundletool.missing" "$apk"
expect "unknown argument" 2 "unknown argument '--sign'" "$bundletool" "$apk" --sign
expect "abi without value" 2 "--abi needs an ABI name" "$bundletool" "$apk" --abi
expect "no arguments" 2 "usage:" "$bundletool"
