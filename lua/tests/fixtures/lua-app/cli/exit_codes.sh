#!/bin/sh
set -u
bin="$1"
failures=0

check() {
  want_code="$1"
  want_out="$2"
  want_err="$3"
  shift 3
  out=$("$bin" "$@" 2>"${TEST_TMPDIR:-/tmp}/stderr")
  code=$?
  err=$(cat "${TEST_TMPDIR:-/tmp}/stderr")
  if [ "$code" != "$want_code" ] || [ "$out" != "$want_out" ] || [ "$err" != "$want_err" ]; then
    echo "FAIL: $* -> $code '$out' '$err' (want $want_code '$want_out' '$want_err')"
    failures=$((failures + 1))
  fi
}

report="lamp 4 x 1250 = 5000
cable 12 x 199 = 2388
fuse 0 x 35 = 0
total 7388"

check 0 "$report" ""
check 0 "$report" "" report
check 0 "cable 12 @ 199" "" get cable
check 4 "" "units: no unit bulb" get bulb
check 2 "" "usage: units [report | --dump | get NAME]" sell
check 2 "" "usage: units [report | --dump | get NAME]" get
check 2 "" "usage: units [report | --dump | get NAME]" report extra

exit "$failures"
