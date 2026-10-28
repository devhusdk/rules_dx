#!/bin/sh
# usage: check.sh <expected-exit> <log-substring-or-->  <report files...>
set -eu
expected="$1"
needle="$2"
shift 2
exit_file=""
log=""
for file in "$@"; do
  case "$file" in
    *.exit) exit_file="$file" ;;
    *.log) log="$file" ;;
  esac
done
[ -n "$exit_file" ] && [ -n "$log" ] || { echo "report lacks a log or exit file: $*" >&2; exit 1; }
actual="$(cat "$exit_file")"
if [ "$actual" != "$expected" ]; then
  echo "expected exit $expected, got $actual" >&2
  cat "$log" >&2
  exit 1
fi
if [ "$needle" != "-" ] && ! grep -q -- "$needle" "$log"; then
  echo "log lacks '$needle'" >&2
  cat "$log" >&2
  exit 1
fi
