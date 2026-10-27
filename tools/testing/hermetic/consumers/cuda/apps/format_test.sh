#!/bin/sh
set -u
tool="$1"
style="$2"
shift 2
failures=0
for src in "$@"; do
  if ! "$tool" "--style=file:$style" --dry-run --Werror "$src"; then
    failures=$((failures + 1))
  fi
done
if [ "$#" = 0 ]; then
  echo "FAIL: no sources to check"
  exit 1
fi
exit "$failures"
