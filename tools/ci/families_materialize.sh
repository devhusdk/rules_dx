#!/usr/bin/env bash
set -euo pipefail

for dir in families/*/; do
  [ -f "$dir/MODULE.bazel" ] || continue
  (cd "$dir" && ../../tools/dx config >/dev/null)
done
