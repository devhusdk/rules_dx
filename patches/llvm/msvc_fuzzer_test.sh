#!/bin/sh
# The MSVC ABI accepts the fuzzer feature, keeps asan unsupported, stages clang_rt.fuzzer*, and compiles compiler-rt against the MSVC headers (#9943).
set -eu
patch="$1"
grep -q '^-    "fuzzer",$' "$patch"
if grep -q '^[-+ ]    "asan",$' "$patch"; then
    echo "$patch touches the asan entry" >&2
    exit 1
fi
grep -q '^+        if sanitizer != "fuzzer"$' "$patch"
grep -q '"clang_rt.fuzzer"' "$patch"
grep -q '"clang_rt.fuzzer_no_main"' "$patch"
grep -q '^+        ":windows_msvc": \[\],$' "$patch"
