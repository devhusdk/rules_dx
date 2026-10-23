#!/bin/sh
set -eu
for tool in makeappx.exe signtool.exe; do
  found=""
  for path in "$@"; do
    case "$path" in
      */"$tool") found="$TEST_SRCDIR/$path" ;;
    esac
  done
  if [ -z "$found" ] || [ "$(head -c 2 "$found")" != "MZ" ]; then
    echo "$tool is missing or not a PE file" >&2
    exit 1
  fi
done
