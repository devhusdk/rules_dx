#!/bin/sh
set -eu

bin="$1"
export TEST_UNDECLARED_OUTPUTS_DIR="$TEST_TMPDIR/outputs"
./go/rules/bench_run.sh "$bin" -test.bench=. -test.run='^$' -test.benchmem -test.benchtime=10x -test.count=1 >/dev/null

json="$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json"
[ -f "$json" ] || { echo "dx-bench.json was not written" >&2; exit 1; }
for metric in "BenchmarkParse ns/op" "BenchmarkParse B/op" "BenchmarkParse allocs/op"; do
    grep -q "\"name\":\"$metric\"" "$json" || { echo "missing $metric in $(cat "$json")" >&2; exit 1; }
done
grep -q '"schema":"dx-bench/v1"' "$json" || { echo "bad schema" >&2; exit 1; }
