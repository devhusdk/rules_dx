#!/bin/sh
set -eu

[ "$(head -c 2 "$1")" != "#!" ] || { echo "$1 is a launcher script, not a native executable" >&2; exit 1; }

export TEST_UNDECLARED_OUTPUTS_DIR="$TEST_TMPDIR/outputs"
mkdir -p "$TEST_UNDECLARED_OUTPUTS_DIR"
"$1" >/dev/null

json="$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json"
[ -f "$json" ] || { echo "dx-bench.json was not written" >&2; exit 1; }
for want in '"schema":"dx-bench/v1"' '"name":"sum"' '"unit":"ns/iter"'; do
    grep -q "$want" "$json" || { echo "missing $want in $(cat "$json")" >&2; exit 1; }
done
