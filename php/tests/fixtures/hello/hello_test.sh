#!/usr/bin/env bash
set -euo pipefail
hello="$1"
shift
out="$("$hello" "$@")"
want=$'Hello, WORLD!\nprecision=7'
if [[ "$out" != "$want" ]]; then
    printf 'unexpected output:\n%s\n' "$out" >&2
    exit 1
fi
