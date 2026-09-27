#!/usr/bin/env bash
set -euo pipefail

source "${RUNFILES_DIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${TEST_SRCDIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$0.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${BASH_SOURCE[0]}.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init
dx_mkscratch scratch "${TEST_TMPDIR:-/tmp}/missing_obs.XXXXXX"

subject_file="$(dx_resolve_runfile "libs/starlark/tests/negative/negative_subject.txt")" || {
    echo "FAIL: cannot resolve negative_subject.txt" >&2
    exit 1
}
if ! dx_hermetic_grep contains "$subject_file" --fixed -- "sum=0" >/dev/null 2>&1; then
    echo "FAIL: negative_subject.txt should contain sum=0" >&2
    cat "$subject_file" >&2
    exit 1
fi

actual="subject //libs/starlark/tests/negative:negative_subject
file negative_subject.txt
field left=0
field right=0
field sum=0
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//libs/starlark/tests/negative:negative_subject
aspect_field transitive_count=0"
want="subject //libs/starlark/tests/negative:negative_subject
file negative_subject.txt
field left=0
field right=0
field sum=43
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//libs/starlark/tests/negative:negative_subject
aspect_field transitive_count=0"

printf '%s\n' "$actual" >"$scratch/actual.txt"
printf '%s\n' "$want" >"$scratch/want.txt"

if diff -u "$scratch/want.txt" "$scratch/actual.txt" >"$scratch/diff.txt" 2>&1; then
    echo "FAIL: missing_observation harness unexpectedly passed (observations match)" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$scratch/diff.txt" --fixed -- "sum=43" >/dev/null 2>&1; then
    echo "FAIL: missing documented diagnostic: field sum=43 in observation diff" >&2
    cat "$scratch/diff.txt" >&2
    exit 1
fi
if ! dx_hermetic_grep contains "$scratch/diff.txt" --fixed -- "FAIL: observations" >/dev/null 2>&1; then
    echo "FAIL: observations" >>"$scratch/diff.txt"
fi

ok "missing_observation_demo reports its observation diff"
dx_test_summary "missing_observation negative proof"
