#!/bin/sh
set -eu
check="$1"
bundletool="$2"
unsigned="$3"
malformed="$4"
signed="$5"
v2_only="$6"
"$check" "$bundletool" "$unsigned" >/dev/null || { echo "check rejects the unsigned fixture"; exit 1; }
reject() {
    label="$1"
    pattern="$2"
    shift 2
    set +e
    out="$("$check" "$bundletool" "$@" 2>&1)"
    code="$?"
    set -e
    [ "$code" != "0" ] || { echo "$label exited 0"; exit 1; }
    echo "$out" | grep -q "$pattern" || { echo "$label misses '$pattern': $out"; exit 1; }
}
reject "malformed package" "not a readable zip" "$malformed"
reject "missing package" "no package at" "$unsigned.missing"
reject "signed package" "already carries a signature" "$signed"
reject "v2 only package" "APK signing block" "$v2_only"
