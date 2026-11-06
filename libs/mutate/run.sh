#!/bin/sh
set -eu

usage="usage: bazel run <name>_run -- [--time <seconds>] --out <dir>"
kind="$1"
package="$2"
shift 2
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

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
rows="$work/rows"

case "$kind" in
    mutmut) tool="${MUTMUT:-}" ;;
    gremlins) tool="${GREMLINS:-gremlins}"; fallback=gremlins ;;
    stryker) tool="${STRYKER:-}" ;;
    pitest) tool="${PITEST:-mvn -q test-compile org.pitest:pitest-maven:1.30.2:mutationCoverage -DtimestampedReports=false -DoutputFormats=XML}" ;;
    *) echo "unknown mutation tool kind: $kind" >&2; exit 2 ;;
esac
# The pinned binary is used only when the tool is not overridden or on PATH.
if [ -n "$bundled" ] && [ "${fallback:-}" = "$tool" ] && ! command -v "$tool" >/dev/null 2>&1; then
    tool="$bundled"
fi

cd "$workspace/$package"

# Without an override, use the tool on PATH or in node_modules, else a pinned version.
if [ -z "$tool" ]; then
    case "$kind" in
        mutmut)
            if command -v mutmut >/dev/null 2>&1; then
                tool=mutmut
            else
                tool="uv run --with mutmut==3.8.1 mutmut"
            fi
            ;;
        stryker)
            if [ -x node_modules/.bin/stryker ]; then
                tool=node_modules/.bin/stryker
            elif command -v stryker >/dev/null 2>&1; then
                tool=stryker
            else
                tool="npx --yes --package @stryker-mutator/core@10.1.1 stryker"
            fi
            ;;
    esac
fi
first="${tool%% *}"
command -v "$first" >/dev/null 2>&1 || [ -x "$first" ] || { echo "$first not found; install it or set $(echo "$kind" | tr a-z A-Z)" >&2; exit 1; }

# Runs the tool within the time budget; a timeout or survivors are not errors.
run_tool() {
    status=0
    if command -v timeout >/dev/null 2>&1; then
        timeout "$seconds" "$@" || status=$?
    else
        "$@" || status=$?
    fi
    [ "$status" != 124 ] || return 0
    return "$status"
}

# Each reader writes one "status<TAB>file<TAB>line<TAB>mutation" row per mutant,
# with status killed, survived or timeout.
case "$kind" in
    mutmut)
        # shellcheck disable=SC2086
        run_tool $tool run || { echo "mutmut failed" >&2; exit 1; }
        # shellcheck disable=SC2086
        $tool results --all true | sed -n -E \
            's/^[[:space:]]*([^[:space:]]+)\.(x[^.[:space:]]*__mutmut_[0-9]+): (killed|survived|timeout)$/\3\t\1\t0\t\2/p' |
            awk -F'\t' -v OFS='\t' '{ gsub(/\./, "/", $2); $2 = $2 ".py"; print }' >"$rows"
        ;;
    gremlins)
        # shellcheck disable=SC2086
        run_tool $tool unleash >"$work/gremlins.txt" || { echo "gremlins failed" >&2; exit 1; }
        sed -n -E 's/^[[:space:]]*(KILLED|LIVED|TIMED OUT) ([A-Z_]+) at ([^:]+):([0-9]+):[0-9]+.*$/\1\t\3\t\4\t\2/p' "$work/gremlins.txt" |
            sed -e 's/^KILLED/killed/' -e 's/^LIVED/survived/' -e 's/^TIMED OUT/timeout/' >"$rows"
        ;;
    stryker)
        # shellcheck disable=SC2086
        run_tool $tool run --reporters json || { echo "stryker failed" >&2; exit 1; }
        node -e '
const r = JSON.parse(require("fs").readFileSync("reports/mutation/mutation.json", "utf8"));
const map = { Killed: "killed", Survived: "survived", NoCoverage: "survived", Timeout: "timeout" };
for (const [file, f] of Object.entries(r.files))
  for (const m of f.mutants)
    if (map[m.status]) console.log([map[m.status], file, m.location.start.line, m.mutatorName].join("\t"));
' >"$rows"
        ;;
    pitest)
        # shellcheck disable=SC2086
        run_tool $tool || { echo "pitest failed" >&2; exit 1; }
        sed -n -E "s#^.*<mutation detected='[a-z]+' status='([A-Z_]+)'.*<sourceFile>([^<]*)</sourceFile>.*<lineNumber>([0-9]+)</lineNumber>.*<description>([^<]*)</description>.*#\1\t\2\t\3\t\4#p" \
            target/pit-reports/mutations.xml |
            sed -e 's/^KILLED/killed/' -e 's/^SURVIVED/survived/' -e 's/^NO_COVERAGE/survived/' \
                -e 's/^TIMED_OUT/timeout/' -e '/^[A-Z]/d' >"$rows"
        ;;
esac

awk -F'\t' '
function esc(s) { gsub(/\\/, "\\\\", s); gsub(/"/, "\\\"", s); return s }
$1 == "killed" { k++ }
$1 == "timeout" { t++ }
$1 == "survived" {
    s++
    list = list (list == "" ? "" : ",") "{\"file\": \"" esc($2) "\", \"line\": " $3 ", \"mutation\": \"" esc($4) "\"}"
}
END { printf "{\"killed\": %d, \"survived\": %d, \"timed_out\": %d, \"survivors\": [%s]}\n", k, s, t, list }
' "$rows" >"$out/mutants.json"
