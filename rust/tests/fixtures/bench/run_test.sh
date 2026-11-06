#!/bin/sh
set -eu

bin="$1"
report="$2"
export TEST_UNDECLARED_OUTPUTS_DIR="$TEST_TMPDIR/outputs"
mkdir -p "$TEST_UNDECLARED_OUTPUTS_DIR"
"$report" "$bin" --bench --output-format bencher >/dev/null

json="$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json"
[ -f "$json" ] || { echo "dx-bench.json was not written" >&2; exit 1; }
for want in '"schema":"dx-bench/v1"' '"name":"parse_small"' '"unit":"ns/iter"'; do
    grep -q "$want" "$json" || { echo "missing $want in $(cat "$json")" >&2; exit 1; }
done
