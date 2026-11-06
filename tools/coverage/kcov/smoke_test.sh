#!/bin/sh
set -eu
kcov="$1"
bin="$2"
out="${TEST_TMPDIR:-/tmp}/kcov_out"
"$kcov" "$out" "$bin"
report=$(find "$out" -name cobertura.xml | head -n 1)
test -n "$report" || { echo "no cobertura.xml under $out" >&2; exit 1; }
grep -q 'hits="[1-9]' "$report" || { echo "no covered line in $report" >&2; cat "$report" >&2; exit 1; }

out2="${TEST_TMPDIR:-/tmp}/kcov_out_lazy"
"$kcov" "--replace-src-path=^.*?/(?=(smoke.c|tools/coverage/kcov/smoke.c)\$):$PWD/" "$out2" "$bin"
report2=$(find "$out2" -name cobertura.xml | head -n 1)
test -n "$report2" || { echo "no cobertura.xml under $out2" >&2; exit 1; }
grep -q 'filename="[^"]*tools/coverage/kcov/smoke.c"' "$report2" || { echo "lazy prefix did not keep the longest source" >&2; cat "$report2" >&2; exit 1; }
