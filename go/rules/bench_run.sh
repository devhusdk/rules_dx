#!/bin/sh
set -eu

test_bin="$1"
shift

out="${TEST_UNDECLARED_OUTPUTS_DIR:?TEST_UNDECLARED_OUTPUTS_DIR is not set}"
log="${TEST_TMPDIR:-${TMPDIR:-/tmp}}/bench.out"

status=0
"$test_bin" "$@" >"$log" || status=$?
cat "$log"
[ "$status" = 0 ] || exit "$status"

mkdir -p "$out"
awk '
function emit(name, unit, value, iterations) {
    printf "%s{\"name\":\"%s %s\",\"unit\":\"%s\",\"mean\":%s,\"stddev\":0,\"iterations\":%s}", (count++ ? "," : ""), name, unit, unit, value, iterations
}
BEGIN { printf "{\"schema\":\"dx-bench/v1\",\"results\":[" }
/^Benchmark/ && NF >= 4 && $2 ~ /^[0-9]+$/ {
    name = $1
    sub(/-[0-9]+$/, "", name)
    for (i = 3; i < NF; i += 2) {
        if ($(i + 1) == "ns/op" || $(i + 1) == "B/op" || $(i + 1) == "allocs/op") {
            emit(name, $(i + 1), $i, $2)
        }
    }
}
END { printf "]}\n"; exit count ? 0 : 3 }
' "$log" >"$out/dx-bench.json" || { echo "bench_run: no benchmark results in the test output" >&2; exit 1; }
