#!/usr/bin/env bash
set -eu
problems="$1"
manual="$2"
fail=0
if [ -s "$problems" ] && [ -n "$(tr -d '\n' <"$problems")" ]; then
  cat "$problems" >&2
  fail=1
fi
while read -r test suites; do
  [ -n "$test" ] || continue
  named=0
  for suite in $suites; do
    if grep -qE "${suite}([^A-Za-z0-9_]|$)" "$3"; then
      named=1
    fi
  done
  if [ "$named" = 0 ]; then
    echo "manual test $test is only in suites CONTRIBUTING.md does not name: $suites" >&2
    fail=1
  fi
done <"$manual"
exit "$fail"
