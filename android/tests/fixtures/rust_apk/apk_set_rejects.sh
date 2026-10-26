#!/bin/sh
set -eu
build="$1"
bundletool="$2"
aab="$3"
spec="$4"
keystore="$5"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
expect() {
    label="$1"
    want="$2"
    pattern="$3"
    shift 3
    set +e
    out="$("$build" "$@" 2>&1)"
    code="$?"
    set -e
    [ "$code" = "$want" ] || { echo "$label exited $code, want $want: $out"; exit 1; }
    echo "$out" | grep -q "^dx-android-apk-set: .*$pattern" || { echo "$label misses '$pattern': $out"; exit 1; }
}
expect "abi mismatch" 1 "does not match .*ABI" "$bundletool" "$aab" "$spec" "$keystore" "$work/a.apks" "$work/a"
expect "missing spec" 2 "no device spec at" "$bundletool" "$aab" "$spec.missing" "$keystore" "$work/b.apks" "$work/b"
expect "missing bundle" 2 "no app bundle at" "$bundletool" "$aab.missing" "$spec" "$keystore" "$work/c.apks" "$work/c"
expect "bad arguments" 2 "usage:" "$bundletool" "$aab"
