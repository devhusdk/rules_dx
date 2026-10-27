#!/usr/bin/env bash
set -euo pipefail
if out="$("$1" 2>&1)"; then
    echo "a class of a package outside deps must not load: $out" >&2
    exit 1
fi
[[ "$out" == *'Psr\Http\Message\RequestInterface'* ]] || { echo "unexpected failure: $out" >&2; exit 1; }
