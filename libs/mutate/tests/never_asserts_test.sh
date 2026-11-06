#!/bin/sh
# A test that never asserts must leave mutants alive; one that asserts must kill them.
set -eu

run="$PWD/libs/mutate/run.sh"
work="$TEST_TMPDIR/work"
mkdir -p "$work/pkg" "$work/bin"
export BUILD_WORKSPACE_DIRECTORY="$work"

cat >"$work/pkg/calc.sh" <<'LIB'
add() { echo $(($1 + $2)); }
LIB
cat >"$work/pkg/lazy_test.sh" <<'T'
. ./calc.sh
add 1 2 >/dev/null
T
cat >"$work/pkg/strict_test.sh" <<'T'
. ./calc.sh
[ "$(add 1 2)" = 3 ]
T

# Stand-in for gremlins: flips the first + to -, runs the package test, reports the outcome.
cat >"$work/bin/gremlins" <<'FAKE'
#!/bin/sh
test_file="$TEST_FILE"
cp calc.sh calc.sh.orig
sed 's/+/-/' calc.sh.orig >calc.sh
if sh "$test_file"; then verdict=LIVED; else verdict=KILLED; fi
mv calc.sh.orig calc.sh
printf '%s ARITHMETIC_BASE at calc.sh:1:21\n' "$verdict"
FAKE
chmod +x "$work/bin/gremlins"

TEST_FILE=lazy_test.sh GREMLINS="$work/bin/gremlins" "$run" gremlins pkg --time 10 --out .dx/lazy
TEST_FILE=strict_test.sh GREMLINS="$work/bin/gremlins" "$run" gremlins pkg --time 10 --out .dx/strict

lazy="$(cat "$work/.dx/lazy/mutants.json")"
strict="$(cat "$work/.dx/strict/mutants.json")"
[ "$lazy" = '{"killed": 0, "survived": 1, "timed_out": 0, "survivors": [{"file": "calc.sh", "line": 1, "mutation": "ARITHMETIC_BASE"}]}' ] ||
    { echo "never-asserting test: unexpected result: $lazy" >&2; exit 1; }
[ "$strict" = '{"killed": 1, "survived": 0, "timed_out": 0, "survivors": []}' ] ||
    { echo "asserting test: unexpected result: $strict" >&2; exit 1; }

# The pinned binary is used when the tool is not on PATH.
cp "$work/bin/gremlins" "$work/pinned"
PATH=/usr/bin:/bin TEST_FILE=lazy_test.sh "$run" gremlins pkg "$work/pinned" --out .dx/pinned
[ "$(cat "$work/.dx/pinned/mutants.json")" = "$lazy" ] || { echo "pinned tool not used" >&2; exit 1; }
