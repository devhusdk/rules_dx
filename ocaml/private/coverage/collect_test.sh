#!/bin/sh
set -eu
collect="$1"
upstream="$2"
dir="${TEST_TMPDIR:-/tmp}/collect_test"
mkdir -p "$dir"
status=0
DX_BISECT_UPSTREAM="$upstream" "$collect" || status=$?
test "$status" = 3 || { echo "expected exit 3 without coverage, got $status" >&2; exit 1; }
status=0
COVERAGE_DIR="$dir/cov" DX_BISECT_UPSTREAM="$upstream" "$collect" || status=$?
test "$status" = 3 || { echo "expected exit 3 with no data, got $status" >&2; exit 1; }
