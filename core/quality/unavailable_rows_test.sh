#!/usr/bin/env bash
set -eu
build="$1"
shift
fail=0
tools="$(awk '
  /^alias\($/ { name = ""; hit = 0 }
  /name = "[a-z_]+_tool"/ { name = $0; sub(/.*name = "/, "", name); sub(/_tool".*/, "", name) }
  /":windows_arm64": ":[a-z_]+_unsupported"/ { if (name != "") print name }
' "$build")"
[ -n "$tools" ] || { echo "FAIL: no unavailable stubs found in $build" >&2; exit 1; }
for tool in $tools; do
  for f in "$@"; do
    bad="$(awk -v id="$tool" '
      /^    tool\($/ { cur = "" }
      /^        id = "/ { cur = $0; sub(/.*id = "/, "", cur); sub(/".*/, "", cur) }
      cur == id && /platforms = \[.*"windows_arm64"/ { print FILENAME ": " $0 }
    ' "$f")"
    if [ -n "$bad" ]; then
      echo "FAIL: $tool resolves to dx_unavailable_tool on windows_arm64 but a row lists it: $bad" >&2
      fail=1
    fi
  done
done
exit "$fail"
