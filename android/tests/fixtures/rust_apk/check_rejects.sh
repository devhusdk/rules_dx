#!/bin/sh
set -eu
check="$1"
unsigned="$2"
malformed="$3"
signed="$4"
"$check" "$unsigned" >/dev/null || { echo "check rejects the unsigned fixture"; exit 1; }
reject() {
    label="$1"
    pattern="$2"
    shift 2
    set +e
    out="$("$check" "$@" 2>&1)"
    code="$?"
    set -e
    [ "$code" != "0" ] || { echo "$label exited 0"; exit 1; }
    echo "$out" | grep -q "$pattern" || { echo "$label misses '$pattern': $out"; exit 1; }
}
reject "malformed package" "not a readable zip" "$malformed"
reject "missing package" "no APK at" "$unsigned.missing"
reject "signed package" "already carries a signature" "$signed"
