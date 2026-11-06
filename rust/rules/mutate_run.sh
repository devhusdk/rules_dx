#!/bin/sh
set -eu

usage="usage: bazel run <name>_run -- [--time <seconds>] --out <dir>"
package="$1"
shift
bundled="${1:-}"
case "$bundled" in
    ""|--*) bundled="" ;;
    /*) shift ;;
    *) bundled="$PWD/$bundled"; shift ;;
esac
[ -x "$bundled" ] && [ -f "$bundled" ] || bundled=""

seconds=60
out=""
while [ $# -gt 0 ]; do
    case "$1" in
        --time) seconds="$2"; shift ;;
        --out) out="$2"; shift ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
    shift
done
[ -n "$out" ] || { echo "$usage" >&2; exit 2; }

workspace="${BUILD_WORKSPACE_DIRECTORY:-$PWD}"
case "$out" in
    /*) ;;
    *) out="$workspace/$out" ;;
esac
mkdir -p "$out"

tool="${CARGO_MUTANTS:-cargo-mutants}"
if [ -n "$bundled" ] && [ "$tool" = cargo-mutants ] && ! command -v "$tool" >/dev/null 2>&1; then
    tool="$bundled"
fi
command -v "$tool" >/dev/null 2>&1 || { echo "cargo-mutants not found; install it or set CARGO_MUTANTS" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cd "$workspace/$package"
status=0
if command -v timeout >/dev/null 2>&1; then
    timeout "$seconds" "$tool" mutants --output "$work" || status=$?
else
    "$tool" mutants --output "$work" || status=$?
fi
case "$status" in
    0|2|3|124) ;;
    *) echo "cargo-mutants failed with exit $status" >&2; exit 1 ;;
esac

results="$work/mutants.out"
count() {
    if [ -f "$results/$1" ]; then wc -l <"$results/$1" | tr -d ' '; else echo 0; fi
}

{
    printf '{"killed": %s, "survived": %s, "timed_out": %s, "survivors": [' \
        "$(count caught.txt)" "$(count missed.txt)" "$(count timeout.txt)"
    if [ -f "$results/missed.txt" ]; then
        sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' \
            -e 's/^\([^:]*\):\([0-9][0-9]*\):[0-9]*: *\(.*\)$/{"file": "\1", "line": \2, "mutation": "\3"}/' \
            "$results/missed.txt" | paste -sd, - | tr -d '\n'
    fi
    printf ']}\n'
} >"$out/mutants.json"
