#!/bin/sh
set -eu

bin="$1"
export TEST_UNDECLARED_OUTPUTS_DIR="$TEST_TMPDIR/outputs"
./cc/private/bench_run.sh "$bin" --benchmark_min_time=10x >/dev/null

json="$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json"
[ -f "$json" ] || { echo "dx-bench.json was not written" >&2; exit 1; }
grep -q '"name":"BM_Parse ns"' "$json" || { echo "missing BM_Parse in $(cat "$json")" >&2; exit 1; }
grep -q '"schema":"dx-bench/v1"' "$json" || { echo "bad schema" >&2; exit 1; }
