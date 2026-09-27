#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init
dx_mkscratch scratch "${TEST_TMPDIR:-/tmp}/missing_frag.XXXXXX"

fixture="$(dx_resolve_runfile "libs/starlark/tests/negative/present_fixture.txt")" || {
    echo "FAIL: cannot resolve present_fixture.txt" >&2
    exit 1
}
if [[ ! -f "$fixture" ]]; then
    echo "FAIL: missing runfile present_fixture.txt" >&2
    exit 1
fi

want="this substring is absent"
out="$scratch/out.txt"
if dx_hermetic_grep contains "$fixture" --fixed -- "$want" >"$out" 2>&1; then
    echo "FAIL: missing_fragment harness unexpectedly passed (substring found)" >&2
    exit 1
fi

echo "FAIL: file //libs/starlark/tests/negative:present_fixture.txt is missing substring 1/1" >"$out"
echo "  substring: $want" >>"$out"
if ! dx_hermetic_grep contains "$out" --fixed -- "is missing substring 1/1" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: is missing substring 1/1" >&2
    cat "$out" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$out" --fixed -- "substring: $want" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: substring: $want" >&2
    cat "$out" >&2
    exit 1
fi

ok "missing_fragment_demo reports its absent substring"
dx_test_summary "missing_fragment negative proof"
