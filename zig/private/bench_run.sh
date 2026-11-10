#!/bin/sh
set -eu

bench_bin="$1"
shift

out="${TEST_UNDECLARED_OUTPUTS_DIR:?TEST_UNDECLARED_OUTPUTS_DIR is not set}"
log="${TEST_TMPDIR:-${TMPDIR:-/tmp}}/bench.txt"

status=0
"$bench_bin" "$@" >"$log" || status=$?
cat "$log"
[ "$status" = 0 ] || exit "$status"

mkdir -p "$out"
awk '
BEGIN { printf "{\"schema\":\"dx-bench/v1\",\"results\":[" }
$1 == "bench" && NF == 4 && $3 ~ /^[0-9.]+$/ && $4 ~ /^[0-9]+$/ {
    printf "%s{\"name\":\"%s ns/op\",\"unit\":\"ns/op\",\"mean\":%s,\"stddev\":0,\"iterations\":%s}", (count++ ? "," : ""), $2, $3, $4
}
END { printf "]}\n"; exit count ? 0 : 3 }
' "$log" >"$out/dx-bench.json" || { echo "bench_run: no bench lines in the output" >&2; exit 1; }
