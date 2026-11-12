#!/bin/sh
set -eu
swift="$1"
source="$TEST_TMPDIR/warn.swift"
cp "$2" "$source"
plain="$TEST_TMPDIR/plain.log"
strict="$TEST_TMPDIR/strict.log"
if ! "$swift" --driver-mode=swiftc -typecheck "$source" >"$plain" 2>&1; then
    cat "$plain" >&2
    echo "the warning alone failed the compile" >&2
    exit 1
fi
cat "$plain"
grep -q "warning: initialization of immutable value 'unused'" "$plain"
if "$swift" --driver-mode=swiftc -typecheck -warnings-as-errors "$source" >"$strict" 2>&1; then
    cat "$strict" >&2
    echo "-warnings-as-errors compiled a warning" >&2
    exit 1
fi
cat "$strict"
grep -q "error: initialization of immutable value 'unused'" "$strict"
