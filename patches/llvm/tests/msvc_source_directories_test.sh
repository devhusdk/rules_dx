#!/bin/sh
# The MSVC header and library subsets come from @msvc_runtime, not from copy actions (#8912).
set -eu
if grep -n copy_to_directory "$1"; then
    echo "$1 still copies MSVC files in build actions" >&2
    exit 1
fi
grep -q '@msvc_runtime//:msvc_vcruntime_include' "$1"
