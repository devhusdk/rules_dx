#!/bin/bash
# Fails when a public binary, test, bench or fuzz wrapper takes positional arguments after name.
set -euo pipefail

status=0
for defs in "$@"; do
    path="$defs"
    bad=$(awk '
        /^def [a-z0-9]+_(binary|test|bench|fuzz_test)\(/ { sig = $0; open = 1 }
        open && !/^def / { sig = sig $0 }
        open && /\):[ \t]*$/ {
            gsub(/[ \t\n]+/, " ", sig); sub(/\( /, "(", sig)
            if (sig !~ /\(name(, \*|,\*)[,)]/ && sig !~ /\(name\)/ && sig !~ /\(name, \*\*/) print sig
            open = 0
        }
    ' "$path")
    if [[ -n "$bad" ]]; then
        echo "$defs: wrappers must be \`name, *, ...\`:" >&2
        echo "$bad" >&2
        status=1
    fi
done
exit "$status"
