#!/usr/bin/env bash
# Devcontainer parity fixture (issue #183, snapshot workflow issue #322):
# the repository's own `.devcontainer/devcontainer.json` is a snapshot of
# the `dx init` scaffold output, so the definition we ship is the one we
# boot. A drifting hand copy is worse than none.
#
# Snapshot testing, not brittle equality: `snapshot_diff` fails with a
# unified diff, while `UPDATE_EXPECT=1` refreshes the golden instead of
# failing (bazel test --test_env=UPDATE_EXPECT). Schema validation below
# pins the contract fields so scaffold drift fails at the source even when
# the snapshot is refreshed.
#
# Shell sources have no corpus class. Process-spawning tests stay out of
# the coverage denominator per the repo coverage preset.
set -euo pipefail

# Shared snapshot helper (issue #322).
source "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/snapshot.sh"

# Shared CI helpers (issues #319, #323).
dx_bootstrap "tools/sh/lib.sh"

# Portable helpers via tools/sh/lib.sh dx_realpath/dx_mkscratch (issues #299, #323).

expected="$(dx_realpath "$1")"
dx_bin="$(dx_realpath "$2")"

dx_mkscratch scratch

# The `dx` binary resolves its workspace through a `MODULE.bazel`
# marker (or `--workspace` override): seed a stub workspace so `init`
# plans against an empty tree instead of this repository.
touch "$scratch/MODULE.bazel"
"${dx_bin}" --workspace "$scratch" init --quiet >/dev/null

actual="$scratch/.devcontainer/devcontainer.json"

# Schema validation (issue #322): pin the contract fields on both the
# scaffold output and the checked-in file, mirroring
# `cli/adopt/src/scaffold.rs::devcontainer_scaffold_runs_bootstrap_not_full_build`.
# Exact bytes stay in the snapshot below; this fails first on shape drift.
for f in "$expected" "$actual"; do
  snapshot_json_validates "$f"
  python3 - "$f" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1]))
assert doc.get("name") == "rules_dx", "name must stay rules_dx"
assert doc.get("image") == "mcr.microsoft.com/devcontainers/base:ubuntu", "image must stay pinned base"
assert "ghcr.io/devcontainers/features/bazel:1" in doc.get("features", {}), "bazel feature must stay"
extensions = doc.get("customizations", {}).get("vscode", {}).get("extensions", [])
assert "rust-lang.rust-analyzer" in extensions, f"rust-analyzer extension must stay: {extensions}"
post = doc.get("postCreateCommand", "")
assert "bazel run //dx:env" in post, f"bootstrap first: {post}"
assert "bazel build //..." not in post, f"no full build on create: {post}"
PY
done
echo "devcontainer schema: scaffold and checked-in definition carry the pinned contract"

snapshot_diff "$expected" "$actual" ".devcontainer/devcontainer.json"
echo "devcontainer parity: scaffold output matches checked-in definition (snapshot)"
