#!/bin/sh
set -eu

bench_bin="$1"
shift

out="${TEST_UNDECLARED_OUTPUTS_DIR:?TEST_UNDECLARED_OUTPUTS_DIR is not set}"
csv="${TEST_TMPDIR:-${TMPDIR:-/tmp}}/bench.csv"

"$bench_bin" --csv "$csv" "$@"

mkdir -p "$out"
awk '
BEGIN { printf "{\"schema\":\"dx-bench/v1\",\"results\":[" }
NR > 1 && match($0, /,[0-9]+,[0-9]+$/) {
    name = substr($0, 1, RSTART - 1)
    split(substr($0, RSTART + 1), nums, ",")
    gsub(/^"|"$/, "", name)
    gsub(/""/, "\\\"", name)
    printf "%s{\"name\":\"%s ns/op\",\"unit\":\"ns/op\",\"mean\":%.3f,\"stddev\":%.3f,\"iterations\":1}", (count++ ? "," : ""), name, nums[1] / 1000, nums[2] / 2000
}
END { printf "]}\n"; exit count ? 0 : 3 }
' "$csv" >"$out/dx-bench.json" || { echo "bench_run: no benchmark results in the csv" >&2; exit 1; }
