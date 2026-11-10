#!/bin/sh
set -eu

driver="$1"
harness="$2"
run="$3"

out="$TEST_TMPDIR/out"
status=0
"$run" dotnet "$driver" "$harness" --time 10 --out "$out" || status=$?
[ "$status" = 77 ] || { echo "runner exit $status, want 77" >&2; exit 1; }
set -- "$out"/crash-*
[ -f "$1" ] || { echo "no crash file in $out" >&2; exit 1; }
status=0
"$run" dotnet "$driver" "$harness" --repro "$1" >/dev/null 2>&1 || status=$?
[ "$status" != 0 ] || { echo "repro did not fail" >&2; exit 1; }
