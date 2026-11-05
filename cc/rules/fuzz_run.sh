#!/bin/sh
set -eu

usage="usage: bazel run <name>_run -- [--time <seconds>] [--out <dir>] [--jobs <n>] [--repro <file>]"
fuzz_bin="$1"
shift

case "$fuzz_bin" in
    /*) ;;
    *) fuzz_bin="$PWD/$fuzz_bin" ;;
esac

seconds=60
out=""
jobs=""
repro=""
while [ $# -gt 0 ]; do
    case "$1" in
        --time) seconds="$2"; shift ;;
        --out) out="$2"; shift ;;
        --jobs) jobs="$2"; shift ;;
        --repro) repro="$2"; shift ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
    shift
done

resolve() {
    case "$1" in
        /*) printf '%s\n' "$1" ;;
        *) printf '%s/%s\n' "${BUILD_WORKSPACE_DIRECTORY:-$PWD}" "$1" ;;
    esac
}

if [ -n "$repro" ]; then
    exec "$fuzz_bin" "$(resolve "$repro")"
fi

[ -n "$out" ] || { echo "$usage" >&2; exit 2; }
out="$(resolve "$out")"
mkdir -p "$out"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/corpus" "$work/artifacts"

parallel=""
[ -z "$jobs" ] || parallel="-fork=$jobs"

status=0
# shellcheck disable=SC2086
"$fuzz_bin" "-max_total_time=$seconds" "-artifact_prefix=$work/artifacts/" $parallel \
    "$work/corpus" || status=$?

found=0
for file in "$work"/artifacts/*; do
    [ -f "$file" ] || continue
    cp "$file" "$out/crash-$(basename "$file")"
    found=1
done
if [ "$found" = 1 ]; then
    exit 77
fi
exit "$status"
