#!/bin/sh
set -eu
readelf="$1"
binary="$2"
dynamic="$("$readelf" --dynamic "$binary")"
echo "$dynamic"
if echo "$dynamic" | grep -E 'RUNPATH|RPATH'; then
    echo "binary has a RUNPATH" >&2
    exit 1
fi
if echo "$dynamic" | grep 'NEEDED' | grep -E 'libswiftCore|external/'; then
    echo "binary needs a Swift runtime or external library" >&2
    exit 1
fi
