#!/bin/sh
set -eu
files=$1
shift
for want in "$@"; do
  text=${want#?}
  found=0
  for f in $files; do
    if grep -aq -- "$text" "$f"; then found=1; fi
  done
  case $want in
    +*) [ "$found" = 1 ] || { echo "missing: $text"; for f in $files; do echo "== $f"; head -c 4000 "$f"; echo; done; exit 1; } ;;
    -*) [ "$found" = 0 ] || { echo "unexpected: $text"; exit 1; } ;;
  esac
done
