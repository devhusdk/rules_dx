#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init
dx_mkscratch scratch "${TEST_TMPDIR:-/tmp}/missing_frag.XXXXXX"

runner="$(dx_resolve_runfile "libs/starlark/tests/negative/red_missing_fragment.sh")" || {
    echo "FAIL: cannot resolve red_missing_fragment.sh" >&2
    exit 1
}

out="$scratch/out.txt"
if "$runner" >"$out" 2>&1; then
    echo "FAIL: red runner unexpectedly passed (proof is void)" >&2
    cat "$out" >&2
    exit 1
fi

if ! dx_hermetic_grep contains "$out" --fixed -- "FAIL: file //libs/starlark/tests/negative:present_fixture.txt is missing substring 1/1" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: is missing substring 1/1" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "substring: this substring is absent" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: substring: this substring is absent" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "starlark_test: 0 passed, 1 failed" >/dev/null 2>&1; then
    echo "FAIL: missing documented summary: starlark_test: 0 passed, 1 failed" >&2
    cat "$out" >&2
    exit 1
fi

ok "missing_fragment_demo reports its absent substring"
dx_test_summary "missing_fragment negative proof"
