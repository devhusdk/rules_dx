#!/bin/sh
# The MSVC ABI accepts fuzzer but not asan, stages clang_rt.fuzzer*, and builds libFuzzer against libc++ without mingw headers (#9943).
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
grep -q '"clang_rt.fuzzer_interceptors"' "$patch"
grep -A2 '^+        ":windows_msvc": \[$' "$patch" > msvc_headers
grep -q '^+            "@llvm-project//libcxx:headers",$' msvc_headers
if grep -q mingw msvc_headers; then
    echo "$patch gives libFuzzer mingw headers on the MSVC ABI" >&2
    exit 1
fi
