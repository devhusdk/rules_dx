#!/bin/sh
set -eu
printf '%s\n' "$@" > "$DX_FAKE_CAPTURE/args.txt"
if [ -n "${DX_FAKE_MAKEAPPX_FAIL:-}" ]; then
  echo "MakeAppx : error: fake failure" >&2
  exit 3
fi
map=""
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    /f) map="$2"; shift ;;
    /p) out="$2"; shift ;;
  esac
  shift
done
cp "$map" "$DX_FAKE_CAPTURE/mapping.txt"
cp "$(dirname "$map")/AppxManifest.xml" "$DX_FAKE_CAPTURE/AppxManifest.xml"
printf 'fake package\n' > "$out"
