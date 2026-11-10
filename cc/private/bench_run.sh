#!/bin/sh
set -eu

bench_bin="$1"
shift

out="${TEST_UNDECLARED_OUTPUTS_DIR:?TEST_UNDECLARED_OUTPUTS_DIR is not set}"
log="${TEST_TMPDIR:-${TMPDIR:-/tmp}}/bench.csv"

status=0
"$bench_bin" --benchmark_format=csv "$@" >"$log" || status=$?
cat "$log"
[ "$status" = 0 ] || exit "$status"

mkdir -p "$out"
awk -F, '
BEGIN { printf "{\"schema\":\"dx-bench/v1\",\"results\":[" }
$1 ~ /^"/ && $2 ~ /^[0-9]+$/ && $5 ~ /^[a-z]+$/ {
    name = $1
    gsub(/"/, "", name)
    printf "%s{\"name\":\"%s %s\",\"unit\":\"%s\",\"mean\":%s,\"stddev\":0,\"iterations\":%s}", (count++ ? "," : ""), name, $5, $5, $3, $2
}
END { printf "]}\n"; exit count ? 0 : 3 }
' "$log" >"$out/dx-bench.json" || { echo "bench_run: no benchmark results in the output" >&2; exit 1; }
