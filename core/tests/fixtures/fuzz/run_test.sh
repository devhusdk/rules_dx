#!/bin/sh
# Runs a fuzz target through the dx fuzz runner and checks that it finds and
# reproduces the planted crash.
# Usage: run_test.sh <runtime rlocationpath> [--seed <file>] <engine> <engine args>...
set -eu

run="$TEST_SRCDIR/$1"
shift
seeds=""
if [ "$1" = --seed ]; then
  # The seed holds the planted crash, so the first run finds it.
  seeds="$(dirname "$2")"
  shift 2
fi

out="$TEST_TMPDIR/out"
status=0
if [ -n "$seeds" ]; then
  "$run" "$@" --time 10 --corpus "$seeds" --out "$out" || status=$?
else
  "$run" "$@" --time 10 --out "$out" || status=$?
fi
[ "$status" = 77 ] || { echo "runner exit $status, want 77" >&2; exit 1; }
crash=""
for f in "$out"/crash-*; do
  crash="$f"
  break
done
[ -f "$crash" ] || { echo "no crash file in $out" >&2; exit 1; }
status=0
"$run" "$@" --repro "$crash" >/dev/null 2>&1 || status=$?
[ "$status" != 0 ] || { echo "repro did not fail" >&2; exit 1; }
