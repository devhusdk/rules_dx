#!/usr/bin/env bash
set -euo pipefail

source "${RUNFILES_DIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${TEST_SRCDIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$0.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${BASH_SOURCE[0]}.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init
dx_mkscratch scratch "${TEST_TMPDIR:-/tmp}/failing_check.XXXXXX"

inner_fail=0
pass_count=0
fail_count=0
check() {
    name="$1"; expected="$2"; actual="$3"
    if [ "$expected" = "$actual" ]; then
        echo "PASS: $name"
        pass_count=$((pass_count + 1))
    else
        echo "FAIL: $name"
        echo "  expected: $expected"
        echo "  actual:   $actual"
        inner_fail=1
        fail_count=$((fail_count + 1))
    fi
}

out="$scratch/out.txt"
{
    check "deliberately wrong sum" "3" "2"
    check "deliberately wrong product" "5" "4"
    check "control that still passes" "2" "2"
    echo "starlark_test: $pass_count passed, $fail_count failed"
} >"$out" 2>&1 || true

if [[ "$inner_fail" != "1" ]]; then
    echo "FAIL: failing_check harness did not observe failure (inner_fail=$inner_fail)" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "FAIL: deliberately wrong sum" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: FAIL: deliberately wrong sum" >&2
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
