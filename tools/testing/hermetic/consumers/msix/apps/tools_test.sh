#!/bin/sh
set -eu
for arch in x64 arm64; do
  case "$arch" in
    x64) want=8664 ;;
    arm64) want=aa64 ;;
  esac
  for tool in makeappx.exe signtool.exe; do
    found=""
    for path in "$@"; do
      case "$path" in
        */"$arch/$tool") found="$TEST_SRCDIR/$path" ;;
      esac
    done
    if [ -z "$found" ] || [ "$(head -c 2 "$found")" != "MZ" ]; then
      echo "$arch $tool is missing or not a PE file" >&2
      exit 1
    fi
    header=$(od -An -tu4 -j60 -N4 "$found" | tr -d ' ')
    machine=$(od -An -tx2 -j$((header + 4)) -N2 "$found" | tr -d ' ')
    if [ "$machine" != "$want" ]; then
      echo "$arch $tool has machine $machine, want $want" >&2
      exit 1
    fi
  done
done
echo "tools_test: x64 and arm64 makeappx and signtool are PE files for their arch"
