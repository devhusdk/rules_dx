#!/bin/sh
set -eu
check="$1"
bundletool="$2"
aab="$3"
pattern="$4"
shift 4
"$check" "$bundletool" "$aab" >/dev/null || { echo "check rejects the bundle without requirements"; exit 1; }
set +e
out="$("$check" "$bundletool" "$aab" "$@" 2>&1)"
code="$?"
set -e
[ "$code" = "1" ] || { echo "check exited $code, want 1: $out"; exit 1; }
echo "$out" | grep -q "^dx-android-check-aab: .*$pattern" || { echo "check misses '$pattern': $out"; exit 1; }
