#!/bin/sh
set -u
failures=0

check() {
  bin="$1"
  want_code="$2"
  want_out="$3"
  want_err="$4"
  shift 4
  out=$("$bin" "$@" 2>"$TEST_TMPDIR/stderr")
  code=$?
  err=$(cat "$TEST_TMPDIR/stderr")
  if [ "$code" != "$want_code" ] || [ "$out" != "$want_out" ] || [ "$err" != "$want_err" ]; then
    echo "FAIL: $bin $* -> $code '$out' '$err' (want $want_code '$want_out' '$want_err')"
    failures=$((failures + 1))
  fi
}

for bin in "$1" "$2"; do
  check "$bin" 0 '24
24
[{"name":"pears","units":24}]' "" "add 2 dozen pears" "count pears"
  check "$bin" 0 '3
[{"name":"café","units":3}]' "" "add 3 café"
  check "$bin" 0 "[]" ""
  check "$bin" 1 "" "error: pears would exceed the limit of 60" "add 6 dozen pears"
  check "$bin" 1 "" "error: unknown command: sell pears" "add 1 pears" "sell pears"
done

exit "$failures"
