#!/bin/sh
set -eu
kcov="$1"
bin="$2"
out="${TEST_TMPDIR:-/tmp}/kcov_out"
"$kcov" "$out" "$bin"
report=$(find "$out" -name cobertura.xml | head -n 1)
test -n "$report" || { echo "no cobertura.xml under $out" >&2; exit 1; }
grep -q 'hits="[1-9]' "$report" || { echo "no covered line in $report" >&2; cat "$report" >&2; exit 1; }
