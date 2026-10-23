#!/bin/sh
set -u
bin="$1"
data=$(dirname "$2")
unset BUILD_WORKING_DIRECTORY LEDGER_FILE
failures=0

fail() {
  echo "FAIL: $*"
  failures=$((failures + 1))
}

check() {
  want_code="$1"
  want_err="$2"
  shift 2
  err=$("$bin" "$@" 2>&1 >/dev/null)
  code=$?
  if [ "$code" != "$want_code" ] || [ "$err" != "$want_err" ]; then
    fail "$* -> $code '$err' (want $want_code '$want_err')"
  fi
}

totals=$("$bin" totals)
want=$(printf 'rent 1200.00\nfood 73.30\nutilities 64.20\ntravel 48.00')
[ "$totals" = "$want" ] || fail "totals -> '$totals'"

version=$("$bin" --version)
[ "$version" = "ledger 2.4.0-consumer" ] || fail "--version -> '$version'"

for args in "" "sell" "total" "--colour list"; do
  "$bin" $args >/dev/null 2>"$TEST_TMPDIR/usage"
  code=$?
  [ "$code" = 2 ] || fail "'$args' exited $code, want 2"
  grep -q "usage: ledger" "$TEST_TMPDIR/usage" || fail "'$args' printed no usage"
done

check 4 "ledger: no category toys" total toys
check 3 "ledger: cannot read ledger file" --file absent.csv list
check 3 "ledger: line 2: date must be YYYY-MM-DD" --file "$data/malformed/bad_date.csv" totals
check 3 'ledger: line 1: header must be "date,category,cents"' --file "$data/malformed/bad_header.csv" totals
check 3 "ledger: ledger has no entries" --file "$data/malformed/empty.csv" totals
check 3 "ledger: line 2: cents must be a positive whole number" --file "$data/malformed/negative.csv" totals
check 3 "ledger: line 2: want 3 fields, got 2" --file "$data/malformed/short_line.csv" totals
check 3 "ledger: line 2: unknown category toys" --file "$data/malformed/unknown_category.csv" totals

food=$(BUILD_WORKING_DIRECTORY="$PWD/$data" "$bin" --file entries.csv total food)
[ "$food" = "food 73.30" ] || fail "relative --file -> '$food'"

exit "$failures"
