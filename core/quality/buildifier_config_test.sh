#!/bin/bash
set -euo pipefail

buildifier="$1"
shift
printf 'load("//a:b.bzl", "c")\n' >"${TEST_TMPDIR}/BUILD.bazel"
for config in "$@"; do
    if ! "${buildifier}" -config="${config}" -mode=check -lint=warn "${TEST_TMPDIR}/BUILD.bazel" >"${TEST_TMPDIR}/out" 2>&1; then
        if grep -q "unexpected warning" "${TEST_TMPDIR}/out"; then
            echo "FAIL: ${config}" >&2
            cut -c1-400 "${TEST_TMPDIR}/out" >&2
            exit 1
        fi
    fi
done
