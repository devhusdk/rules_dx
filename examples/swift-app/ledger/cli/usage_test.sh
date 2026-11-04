#!/bin/sh
set -eu
binary="$1"
code=0
out=$("$binary" "tea=abc" 2>&1) || code=$?
[ "$code" = 64 ] || { echo "exit $code, want 64: $out" >&2; exit 1; }
case "$out" in
*"bad entry 'tea=abc'"*"Usage: ledger"*) ;;
*) echo "missing usage text: $out" >&2; exit 1 ;;
esac
