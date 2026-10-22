#!/bin/sh
set -u
bin="$1"
tmp="${TEST_TMPDIR:-/tmp}/grades_exit_codes"
mkdir -p "$tmp"
failures=0

check() {
  want_code="$1"
  want_err="$2"
  shift 2
  err=$("$bin" "$@" 2>&1 >/dev/null)
  code=$?
  if [ "$code" != "$want_code" ] || [ "$err" != "$want_err" ]; then
    echo "FAIL: $* -> $code '$err' (want $want_code '$want_err')"
    failures=$((failures + 1))
  fi
}

bad() {
  printf '%s\n' "$2" >"$tmp/$1.json"
  check 3 "grades: $3" --file "$tmp/$1.json" list
}

range="score must be a whole number from 0 to 100"
bad above_max '[{"name":"ada","scores":[101]}]' "$range"
bad negative '[{"name":"ada","scores":[-1]}]' "$range"
bad wrong_type '[{"name":"ada","scores":["ninety"]}]' "score must be a number"
bad empty '[{"name":"ada","scores":[]}]' "scores must not be empty"
bad not_array '{"name":"ada"}' "grade file must hold an array"
bad not_json 'this is not json {' "grade file is not valid JSON"
bad single_quoted "[{'name':'ada','scores':[1]}]" "grade file is not valid JSON"

check 4 "grades: no student zed" show zed
check 3 "grades: cannot read grade file" --file "$tmp/absent.json" list

ranks=$("$bin" ranks)
if [ "$ranks" != "$(printf '1 cy 96.7 A\n2 ada 91.7 A\n3 bo 72.3 C')" ]; then
  echo "FAIL: ranks -> '$ranks'"
  failures=$((failures + 1))
fi

"$bin" sell >/dev/null 2>&1
if [ "$?" != 2 ]; then
  echo "FAIL: unknown command did not exit 2"
  failures=$((failures + 1))
fi

exit "$failures"
