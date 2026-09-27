#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init
dx_mkscratch scratch "${TEST_TMPDIR:-/tmp}/failing_check.XXXXXX"

runner="$(dx_resolve_runfile "libs/starlark/tests/negative/red_failing_checks.sh")" || {
    echo "FAIL: cannot resolve red_failing_checks.sh" >&2
    exit 1
}

out="$scratch/out.txt"
if "$runner" >"$out" 2>&1; then
    echo "FAIL: red runner unexpectedly passed (proof is void)" >&2
    cat "$out" >&2
    exit 1
fi

if ! dx_hermetic_grep contains "$out" --fixed -- "FAIL: deliberately wrong sum" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: FAIL: deliberately wrong sum" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "  expected: 3" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: expected: 3" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "  actual:   2" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: actual: 2" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "FAIL: deliberately wrong product" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: FAIL: deliberately wrong product" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "starlark_test: 1 passed, 2 failed" >/dev/null 2>&1; then
    echo "FAIL: missing documented summary: starlark_test: 1 passed, 2 failed" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "PASS: control that still passes" >/dev/null 2>&1; then
    echo "FAIL: control check should still pass" >&2
    cat "$out" >&2
    exit 1
fi

ok "failing_check_demo reports its wrong expects"
dx_test_summary "failing_check negative proof"
