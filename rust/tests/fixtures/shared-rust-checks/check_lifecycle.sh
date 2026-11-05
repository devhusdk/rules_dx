#!/bin/sh
# Checks a logcat capture of the shared-rust activity: lifecycle order, board, input.
set -eu
[ "$#" -ge 1 ] || { echo "usage: check_lifecycle.sh <logcat_file> [--expect-fail]" >&2; exit 2; }
file="$1"
if [ "${2:-}" = "--expect-fail" ]; then
    if sh "$0" "$file" > /dev/null 2>&1; then
        echo "check_lifecycle: $file unexpectedly passed" >&2
        exit 1
    fi
    echo "check_lifecycle: rejected as expected"
    exit 0
fi
[ -f "$file" ] || { echo "check_lifecycle: no logcat file $file" >&2; exit 2; }
awk '
    BEGIN { n = split("create start resume pause stop destroy", seq, " "); k = 1 }
    {
        at = index($0, "DxSharedTasks")
        if (at == 0) next
        count = split(substr($0, at), tok, /[[:space:]]+/)
        for (i = 1; i <= count; i++) {
            if (tok[i] ~ /^lifecycle=/) {
                if (k <= n && substr(tok[i], 11) == seq[k]) k++
            } else if (tok[i] == "board=loaded") loaded = 1
            else if (tok[i] == "board=invalid") invalid = 1
            else if (tok[i] == "input=touch") touch = 1
            else if (tok[i] == "input=key") key = 1
        }
    }
    END {
        fail = 0
        if (k <= n) { print "check_lifecycle: lifecycle sequence stops before: " seq[k] > "/dev/stderr"; fail = 1 }
        if (!loaded || invalid) { print "check_lifecycle: board was not loaded" > "/dev/stderr"; fail = 1 }
        if (!touch) { print "check_lifecycle: no input=touch line" > "/dev/stderr"; fail = 1 }
        if (!key) { print "check_lifecycle: no input=key line" > "/dev/stderr"; fail = 1 }
        exit fail
    }
' "$file"
echo "check_lifecycle: ok"
