#!/usr/bin/env bash
# Reads all of stdin, then exits 0 if it contains the fixed string $1, else 1.
set -euo pipefail
marker=${1:?usage: has_marker.sh <marker>}
text=$(cat)
case "$text" in
  *"$marker"*) exit 0 ;;
  *) exit 1 ;;
esac
