#!/usr/bin/env bash
set -eu
problems="$1"
fail=0
if [ -s "$problems" ] && [ -n "$(tr -d '\n' <"$problems")" ]; then
  cat "$problems" >&2
  fail=1
fi
exit "$fail"
