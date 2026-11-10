#!/bin/sh
set -eu

runner="$1"
bin="$2"
export TEST_UNDECLARED_OUTPUTS_DIR="$TEST_TMPDIR/outputs"
"$runner" "$bin" >/dev/null

json="$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json"
[ -f "$json" ] || { echo "dx-bench.json was not written" >&2; exit 1; }
grep -q '"name":"parse ns/op"' "$json" || { echo "missing parse in $(cat "$json")" >&2; exit 1; }
grep -q '"schema":"dx-bench/v1"' "$json" || { echo "bad schema" >&2; exit 1; }
