#!/bin/sh
set -eu

usage="usage: bazel run <name>_run -- [--time <seconds>] [--out <dir>] [--jobs <n>] [--repro <file>]"
engine="$1"
fuzz_bin="$2"
target="$3"
shift 3

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

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
crashes="$work/artifacts"

case "$engine" in
    libfuzzer|jazzer) mkdir -p "$work/corpus" "$crashes" ;;
    go)
        crashes="$work/testdata/fuzz/$target"
        mkdir -p "$crashes"
        ;;
    *) echo "unknown fuzz engine: $engine" >&2; exit 2 ;;
esac

if [ -n "$repro" ]; then
    case "$engine" in
        libfuzzer) exec "$fuzz_bin" "$(resolve "$repro")" ;;
        jazzer) exec "$fuzz_bin" "--target_class=$target" "$(resolve "$repro")" ;;
        go)
            cp "$(resolve "$repro")" "$crashes/"
            cd "$work"
            exec "$fuzz_bin" "-test.run=^$target\$"
            ;;
    esac
fi

[ -n "$out" ] || { echo "$usage" >&2; exit 2; }
out="$(resolve "$out")"
mkdir -p "$out"

status=0
# shellcheck disable=SC2086
case "$engine" in
    libfuzzer)
        parallel=""
        [ -z "$jobs" ] || parallel="-fork=$jobs"
        "$fuzz_bin" "-max_total_time=$seconds" "-artifact_prefix=$crashes/" $parallel \
            "$work/corpus" || status=$?
        ;;
    jazzer)
        parallel=""
        [ -z "$jobs" ] || parallel="-fork=$jobs"
        # Jazzer writes its Crash_*.java reproducers to the working directory.
        cd "$work"
        "$fuzz_bin" "--target_class=$target" "-max_total_time=$seconds" \
            "-artifact_prefix=$crashes/" $parallel "$work/corpus" || status=$?
        ;;
    go)
        parallel=""
        [ -z "$jobs" ] || parallel="-test.parallel=$jobs"
        mkdir -p "$work/cache"
        cd "$work"
        "$fuzz_bin" "-test.fuzz=^$target\$" -test.run='^$' "-test.fuzztime=${seconds}s" \
            "-test.fuzzcachedir=$work/cache" $parallel || status=$?
        ;;
esac

found=0
for file in "$crashes"/*; do
    [ -f "$file" ] || continue
    cp "$file" "$out/crash-$(basename "$file")"
    found=1
done
if [ "$found" = 1 ]; then
    exit 77
fi
exit "$status"
