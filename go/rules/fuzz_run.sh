#!/bin/sh
set -eu

usage="usage: bazel run <name>_run -- [--time <seconds>] [--out <dir>] [--jobs <n>] [--repro <file>]"
test_bin="$1"
fuzz="$2"
shift 2

case "$test_bin" in
    /*) ;;
    *) test_bin="$PWD/$test_bin" ;;
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

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
corpus="$work/testdata/fuzz/$fuzz"
mkdir -p "$corpus"
cd "$work"

if [ -n "$repro" ]; then
    cp "$(resolve "$repro")" "$corpus/"
    exec "$test_bin" "-test.run=^$fuzz\$"
fi

[ -n "$out" ] || { echo "$usage" >&2; exit 2; }
out="$(resolve "$out")"
mkdir -p "$out" "$work/cache"

parallel=""
[ -z "$jobs" ] || parallel="-test.parallel=$jobs"

status=0
# shellcheck disable=SC2086
"$test_bin" "-test.fuzz=^$fuzz\$" -test.run='^$' "-test.fuzztime=${seconds}s" \
    "-test.fuzzcachedir=$work/cache" $parallel || status=$?

found=0
for file in "$corpus"/*; do
    [ -f "$file" ] || continue
    cp "$file" "$out/crash-$(basename "$file")"
    found=1
done
if [ "$found" = 1 ]; then
    exit 77
fi
exit "$status"
