#!/usr/bin/env bash
# Hermetic smoke test for cpansa_dump: a version line, then the database JSON.
set -u
bin="$PWD/$1"
out=$("$bin") || { echo "cpansa_dump failed: $?" >&2; exit 1; }
version=${out%%$'\n'*}
rest=${out#*$'\n'}
case "$version" in
[0-9]*) ;;
*) echo "bad version line: $version" >&2; exit 1 ;;
esac
case "$rest" in
'{'*) ;;
*) echo "database is not a JSON object: ${rest:0:80}" >&2; exit 1 ;;
esac
