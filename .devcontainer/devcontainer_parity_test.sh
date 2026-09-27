#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/snapshot.sh"
dx_bootstrap "tools/sh/lib.sh"

expected="$(dx_realpath "$1")"
dx_bin="$(dx_realpath "$2")"

dx_mkscratch scratch

touch "$scratch/MODULE.bazel"
"${dx_bin}" --workspace "$scratch" init --quiet >/dev/null

actual="$scratch/.devcontainer/devcontainer.json"

for f in "$expected" "$actual"; do
  snapshot_json_validates "$f"
done

snapshot_diff "$expected" "$actual" ".devcontainer/devcontainer.json"
echo "devcontainer parity: scaffold output matches checked-in definition (snapshot)"
