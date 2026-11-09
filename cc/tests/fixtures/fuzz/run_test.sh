#!/bin/sh
set -eu

run=./tools/fuzz/fuzz_run.sh
bin="$1"

out="$TEST_TMPDIR/out"
status=0
"$run" "$bin" --time 10 --out "$out" || status=$?
[ "$status" = 77 ] || { echo "runner exit $status, want 77" >&2; exit 1; }
set -- "$out"/crash-*
[ -f "$1" ] || { echo "no crash file in $out" >&2; exit 1; }
status=0
"$run" "$bin" --repro "$1" >/dev/null 2>&1 || status=$?
[ "$status" != 0 ] || { echo "repro did not fail" >&2; exit 1; }
