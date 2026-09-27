#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init
dx_mkscratch scratch "${TEST_TMPDIR:-/tmp}/missing_obs.XXXXXX"

runner="$(dx_resolve_runfile "libs/starlark/tests/negative/red_missing_observation.sh")" || {
    echo "FAIL: cannot resolve red_missing_observation.sh" >&2
    exit 1
}

out="$scratch/out.txt"
if "$runner" >"$out" 2>&1; then
    echo "FAIL: red runner unexpectedly passed (proof is void)" >&2
    cat "$out" >&2
    exit 1
fi

if ! dx_hermetic_grep contains "$out" --fixed -- "FAIL: observations" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: FAIL: observations" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "field sum=43" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: field sum=43 in runner output" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "field sum=0" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: field sum=0 in runner output" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "starlark_test: 0 passed, 1 failed" >/dev/null 2>&1; then
    echo "FAIL: missing documented summary: starlark_test: 0 passed, 1 failed" >&2
    cat "$out" >&2
    exit 1
fi

ok "missing_observation_demo reports its observation diff"
dx_test_summary "missing_observation negative proof"
