#!/bin/sh
set -eu

bin="$1"
export TEST_UNDECLARED_OUTPUTS_DIR="$TEST_TMPDIR/outputs"
./haskell/private/bench_run.sh "$bin" --stdev 100 >/dev/null

json="$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json"
[ -f "$json" ] || { echo "dx-bench.json was not written" >&2; exit 1; }
grep -q '"name":"All.sum ns/op"' "$json" || { echo "missing All.sum in $(cat "$json")" >&2; exit 1; }
grep -q '"schema":"dx-bench/v1"' "$json" || { echo "bad schema" >&2; exit 1; }
