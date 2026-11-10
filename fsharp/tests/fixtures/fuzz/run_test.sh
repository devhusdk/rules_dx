#!/bin/sh
set -eu

driver="$1"
run="$2"
shift 2

for harness in "$@"; do
    out="$TEST_TMPDIR/out-$(basename "$harness")"
    status=0
    "$run" dotnet "$driver" "$harness" --time 60 --out "$out" || status=$?
    [ "$status" = 77 ] || { echo "runner exit $status, want 77 for $harness" >&2; exit 1; }
    set -- "$out"/crash-*
    [ -f "$1" ] || { echo "no crash file in $out for $harness" >&2; exit 1; }
    status=0
    "$run" dotnet "$driver" "$harness" --repro "$1" >/dev/null 2>&1 || status=$?
    [ "$status" != 0 ] || { echo "repro of $harness did not fail" >&2; exit 1; }
done
