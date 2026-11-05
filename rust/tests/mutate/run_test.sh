#!/bin/sh
set -eu

run="$PWD/rust/rules/mutate_run.sh"
work="$TEST_TMPDIR/work"
mkdir -p "$work/pkg" "$work/bin"

cat >"$work/bin/cargo-mutants" <<'FAKE'
#!/bin/sh
while [ $# -gt 0 ]; do
    [ "$1" = --output ] && out="$2"
    shift
done
mkdir -p "$out/mutants.out"
printf 'src/lib.rs:3:5: replace add -> i32 with 0\nsrc/lib.rs:9:1: replace "x" with y\n' >"$out/mutants.out/missed.txt"
printf 'src/lib.rs:4:5: replace sub -> i32 with 1\n' >"$out/mutants.out/caught.txt"
exit 2
FAKE
chmod +x "$work/bin/cargo-mutants"

BUILD_WORKSPACE_DIRECTORY="$work" CARGO_MUTANTS="$work/bin/cargo-mutants" \
    "$run" pkg --time 5 --out .dx/mutate/pkg/

json="$work/.dx/mutate/pkg/mutants.json"
expected='{"killed": 1, "survived": 2, "timed_out": 0, "survivors": [{"file": "src/lib.rs", "line": 3, "mutation": "replace add -> i32 with 0"},{"file": "src/lib.rs", "line": 9, "mutation": "replace \"x\" with y"}]}'
[ "$(cat "$json")" = "$expected" ] || { echo "unexpected mutants.json:" >&2; cat "$json" >&2; exit 1; }

status=0
CARGO_MUTANTS=missing-tool "$run" pkg --out "$work/o2" 2>/dev/null || status=$?
[ "$status" = 1 ] || { echo "missing tool exit $status, want 1" >&2; exit 1; }
