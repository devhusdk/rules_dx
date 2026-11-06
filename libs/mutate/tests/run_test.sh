#!/bin/sh
set -eu

run="$PWD/libs/mutate/run.sh"
work="$TEST_TMPDIR/work"
mkdir -p "$work/pkg/reports/mutation" "$work/pkg/target/pit-reports" "$work/bin"
export BUILD_WORKSPACE_DIRECTORY="$work"

fake() {
    printf '#!/bin/sh\n%s\n' "$2" >"$work/bin/$1"
    chmod +x "$work/bin/$1"
}

check() {
    got="$(cat "$work/.dx/$1/mutants.json")"
    [ "$got" = "$2" ] || { echo "$1: unexpected mutants.json: $got" >&2; exit 1; }
}

fake mutmut 'if [ "$1" = results ]; then
printf "    app.core.x_add__mutmut_1: survived\n    app.core.x_sub__mutmut_1: killed\n    app.core.xǁKǁrun__mutmut_2: timeout\n"
fi'
MUTMUT="$work/bin/mutmut" "$run" mutmut pkg --time 5 --out .dx/mutmut
check mutmut '{"killed": 1, "survived": 1, "timed_out": 1, "survivors": [{"file": "app/core.py", "line": 0, "mutation": "x_add__mutmut_1"}]}'

fake gremlins 'printf "     KILLED ARITHMETIC_BASE at a/b.go:3:5\n      LIVED CONDITIONALS_NEGATION at a/b.go:9:2\nTIMED OUT INCREMENT_DECREMENT at a/c.go:1:1\n"'
GREMLINS="$work/bin/gremlins" "$run" gremlins pkg --out .dx/gremlins
check gremlins '{"killed": 1, "survived": 1, "timed_out": 1, "survivors": [{"file": "a/b.go", "line": 9, "mutation": "CONDITIONALS_NEGATION"}]}'

fake stryker 'cat >reports/mutation/mutation.json <<JSON
{"files": {"src/a.ts": {"mutants": [
 {"mutatorName": "ArithmeticOperator", "status": "Survived", "location": {"start": {"line": 4}}},
 {"mutatorName": "StringLiteral", "status": "Killed", "location": {"start": {"line": 5}}},
 {"mutatorName": "BlockStatement", "status": "NoCoverage", "location": {"start": {"line": 8}}},
 {"mutatorName": "Ignored", "status": "Ignored", "location": {"start": {"line": 9}}}]}}}
JSON'
if command -v node >/dev/null 2>&1; then
    STRYKER="$work/bin/stryker" "$run" stryker pkg --out .dx/stryker
    check stryker '{"killed": 1, "survived": 2, "timed_out": 0, "survivors": [{"file": "src/a.ts", "line": 4, "mutation": "ArithmeticOperator"},{"file": "src/a.ts", "line": 8, "mutation": "BlockStatement"}]}'
fi

fake pitest 'cat >target/pit-reports/mutations.xml <<XML
<mutations>
<mutation detected='"'"'true'"'"' status='"'"'KILLED'"'"' numberOfTestsRun='"'"'1'"'"'><sourceFile>A.java</sourceFile><mutatedClass>p.A</mutatedClass><lineNumber>7</lineNumber><description>negated conditional</description></mutation>
<mutation detected='"'"'false'"'"' status='"'"'SURVIVED'"'"' numberOfTestsRun='"'"'1'"'"'><sourceFile>A.java</sourceFile><mutatedClass>p.A</mutatedClass><lineNumber>12</lineNumber><description>replaced return with 0</description></mutation>
<mutation detected='"'"'false'"'"' status='"'"'RUN_ERROR'"'"'><sourceFile>A.java</sourceFile><lineNumber>13</lineNumber><description>x</description></mutation>
</mutations>
XML'
PITEST="$work/bin/pitest" "$run" pitest pkg --out .dx/pitest
check pitest '{"killed": 1, "survived": 1, "timed_out": 0, "survivors": [{"file": "A.java", "line": 12, "mutation": "replaced return with 0"}]}'

status=0
GREMLINS=missing-tool "$run" gremlins pkg --out "$work/o2" 2>/dev/null || status=$?
[ "$status" = 1 ] || { echo "missing tool exit $status, want 1" >&2; exit 1; }

# Without an override or an installed tool, the pinned versions run.
rm -f "$work/bin/mutmut"
fake uv 'printf "%s\n" "$*" >"$(dirname "$0")/uv.args"; printf "    app.x_add__mutmut_1: survived\n"'
PATH="$work/bin:/usr/bin:/bin" "$run" mutmut pkg --out .dx/uv
grep -q 'mutmut==3.8.1' "$work/bin/uv.args" || { echo "uv args: $(cat "$work/bin/uv.args")" >&2; exit 1; }
